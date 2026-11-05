<script lang="ts">
	import { focusOn, retry } from '#lib/ui/focus.ts';
	import { showWhole } from '#lib/ui/landing.ts';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import FirstStep from '#lib/ui/FirstStep.svelte';
	import { behindFade, follow } from '#lib/ui/hold-room.ts';
	import { refreshCounts, reloadOnMoved } from '#lib/nav/counts.svelte.ts';
	import { onMount, tick, untrack } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import {
		arrive,
		DURATION,
		EASE,
		followContent,
		placeholder,
		rise,
		swapIn,
		vanish
	} from '#lib/motion.ts';
	import { confetti } from '#lib/ui/confetti.ts';
	import { announce, notify } from '#lib/ui/notices.svelte.ts';
	import Filters, { type Kind } from '#lib/uncategorized/Filters.svelte';
	import Offer from '#lib/uncategorized/Offer.svelte';
	import Row from '#lib/uncategorized/Row.svelte';
	import { prefersReducedMotion } from 'svelte/motion';
	import { fade, slide } from 'svelte/transition';

	let transactions = $state<Schemas['Transaction'][]>([]);
	let categories = $state<Schemas['Category'][]>([]);
	let accounts = $state<Schemas['Account'][]>([]);
	let loadError = $state('');
	let loaded = $state(false);
	// Nothing imported yet reads differently from everything sorted.
	let empty = $state(false);
	let busy = $state<string | null>(null);

	// The offer opens in place and closes the same way; with reduced motion it only fades.
	const open = (node: Element) =>
		prefersReducedMotion.current
			? fade(node, { duration: DURATION.short })
			: slide(node, { duration: DURATION.medium, easing: EASE.standard });

	/** `arriving` runs as the new list comes, in the same flush. */
	async function load(arriving?: () => void) {
		loadError = '';
		const [list, kinds, any, held] = await Promise.all([
			api.GET('/api/v1/transactions', { params: { query: { uncategorized: true, limit: 1000 } } }),
			api.GET('/api/v1/categories'),
			api.GET('/api/v1/transactions', { params: { query: { limit: 1 } } }),
			api.GET('/api/v1/accounts')
		]);
		if (!list.data || !kinds.data) {
			const failed = list.data ? kinds : list;
			loadError = problem(failed.error, failed.response?.status, t('uncategorized.failed'));
			return;
		}
		arriving?.();
		transactions = list.data.items;
		categories = kinds.data.items;
		empty = any.data?.items.length === 0;
		accounts = held.data?.items ?? [];
		loaded = true;
	}

	let queue = $state<HTMLElement>();
	// Held while the row before leaves: the page glides once to the row that takes the focus, and the list gives back
	// the room after. Let go at once at the page's end, the page jumped 190 px in one frame (pass 15).
	let holding = $state(false);

	// The last row sorted: it stays until the next choice, with Deshacer and, until answered, the offer of a rule.
	let sorted = $state<{ id: string; category: string; name: string; offering: boolean } | null>(
		null
	);
	// A row undone under a filter it does not match: it stays with its select, the way to choose again, until the next
	// choice or filter. It left at once, and the focus fell to the page (pass 40).
	let kept = $state<string | null>(null);
	let kind = $state<Kind>('all');
	let byAmount = $state(false);
	// What the list shows: the filters' answer, a fade behind them (`swap`).
	let view = $state<{ kind: Kind; byAmount: boolean }>({ kind: 'all', byAmount: false });

	const amount = (transaction: Schemas['Transaction']) => transaction.legs[0]?.amount_minor ?? 0;
	const waiting = $derived(transactions.filter((x) => x.id !== sorted?.id));
	const counts = $derived({
		all: waiting.length,
		out: waiting.filter((x) => amount(x) < 0).length,
		in: waiting.filter((x) => amount(x) >= 0).length
	});
	// The sorted row stays under any filter: it is what the person is looking at.
	const shown = $derived.by(() => {
		const wanted = transactions.filter(
			(x) =>
				x.id === sorted?.id ||
				x.id === kept ||
				view.kind === 'all' ||
				amount(x) < 0 === (view.kind === 'out')
		);
		return view.byAmount
			? wanted.toSorted((a, b) => Math.abs(amount(b)) - Math.abs(amount(a)))
			: wanted;
	});

	const list = behindFade(() => queue);

	/** The filters' answer comes behind a fade. Asked again meanwhile, this run goes on to the last one asked. */
	async function swap() {
		if (list.going()) return;
		while (view.kind !== kind || view.byAmount !== byAmount) {
			await list.run(() => {
				kept = null;
				view = { kind, byAmount };
			});
		}
	}
	$effect(() => {
		if (kind !== view.kind || byAmount !== view.byAmount) untrack(swap);
	});

	/** The next row shown that waits, after `id`, then before it. */
	function nextWaiting(id: string) {
		const at = shown.findIndex((x) => x.id === id);
		const after = [...shown.slice(at + 1), ...shown.slice(0, at).reverse()];
		return after.find((x) => x.id !== sorted?.id);
	}

	/** The focus goes on to the next choice, one key away, and the page shows that row once it settles. */
	async function goOn(from: string) {
		await tick();
		const next = nextWaiting(from);
		const target = next
			? focusOn(`#category-${next.id}`)
			: focusOn(`#undo-${from}`, '#all-done a', 'main h1');
		const row = target?.closest('li');
		if (target) await showWhole(row ?? target, row ?? target, queue);
		holding = false;
	}

	async function choose(
		transaction: Schemas['Transaction'],
		categoryId: string,
		select: HTMLSelectElement
	) {
		if (!categoryId || busy) return;
		busy = transaction.id;
		const {
			data,
			error: failure,
			response
		} = await api.PATCH('/api/v1/transactions/{transaction_id}', {
			params: { path: { transaction_id: transaction.id } },
			body: { category_id: categoryId }
		});
		busy = null;
		if (!data) {
			select.value = '';
			notify(problem(failure, response?.status, t('uncategorized.set_failed')), 'bad');
			return;
		}
		const name = categories.find((c) => c.id === categoryId)?.name ?? '';
		announce(t('uncategorized.done', { description: transaction.description, category: name }));
		refreshCounts();
		holding = true;
		// The row before has had its time: it leaves, and this one takes its place as the last sorted.
		const before = sorted?.id;
		if (before) transactions = transactions.filter((x) => x.id !== before);
		kept = null;
		// Under reduced motion nothing grows: it opens at once and the rows below fade to their place under it.
		const offering = prefersReducedMotion.current;
		sorted = { id: transaction.id, category: categoryId, name, offering };
		if (waiting.length === 0) confetti('sorted');
		// The offer opens once the list has placed its rows: opened with them, the rows below measured their place with it
		// whole, went back up over it and slid down as it grew.
		await tick();
		if (sorted?.id === transaction.id) sorted.offering = true;
		await goOn(transaction.id);
	}

	async function undo(transaction: Schemas['Transaction']) {
		if (busy) return;
		busy = transaction.id;
		const {
			data,
			error: failure,
			response
		} = await api.PATCH('/api/v1/transactions/{transaction_id}', {
			params: { path: { transaction_id: transaction.id } },
			body: { category_id: null }
		});
		busy = null;
		if (!data) {
			notify(problem(failure, response?.status, t('uncategorized.undo_failed')), 'bad');
			return;
		}
		announce(t('uncategorized.undone', { description: transaction.description }));
		refreshCounts();
		kept = transaction.id;
		sorted = null;
		await tick();
		// Undone before its fade ended, the row gets the same select back, still on the choice.
		const select = focusOn(`#category-${transaction.id}`);
		if (select instanceof HTMLSelectElement) select.value = '';
	}

	/** «Solo esta vez»: the offer closes and the row keeps its Deshacer until the next choice. */
	async function close(id: string) {
		if (sorted) sorted.offering = false;
		holding = true;
		await goOn(id);
	}

	/** The rule's changes went in: what they sorted leaves the list behind a fade, and the sorted row with it, in the same
	 * flush as the new list: let go first, its tint still changing, it stayed in the flow and the rows jumped after it. */
	async function applied() {
		let target = null as ReturnType<typeof focusOn>;
		await list.run(async () => {
			await load(() => {
				sorted = null;
				kept = null;
			});
			await tick();
			const first = shown[0];
			target = first ? focusOn(`#category-${first.id}`) : focusOn('#all-done a', 'main h1');
		});
		if (target) await showWhole(target);
	}

	onMount(load);
	reloadOnMoved();
</script>

<svelte:head>
	<title>{t('app.title', { page: t('uncategorized.title') })}</title>
</svelte:head>

<h1>{t('uncategorized.title')}</h1>
<p class="hint">{t('uncategorized.hint')}</p>

{#if loadError}
	<p class="alert" role="alert" in:arrive out:placeholder>
		{loadError}
		<button class="secondary" onclick={(event) => retry(event.currentTarget, load)}
			>{t('app.try_again')}</button
		>
	</p>
{:else if loaded}
	<!-- The queue and its «all done» line take each other's place, the one fading over the other. -->
	<div class="in-place" in:arrive>
		{#if transactions.length === 0}
			<!-- Global: these blocks come with the one above them, where a local transition does not play. -->
			{#if empty}
				<!-- Nothing imported yet: the step that comes first, as Summary, Import and Transfers show it. -->
				<div in:swapIn|global>
					<FirstStep {accounts} />
				</div>
			{:else}
				<div id="all-done" in:swapIn|global>
					<EmptyState
						icon="check"
						text={t('uncategorized.all_done')}
						href="/"
						action={t('uncategorized.to_summary')}
					/>
				</div>
			{/if}
		{:else}
			<div class="stack" out:vanish>
				<Filters bind:kind bind:byAmount {counts} />
				<!-- With no category to choose, the way on comes first and the cards hold no control that cannot work. -->
				{#if categories.length === 0}
					<EmptyState
						icon="rules"
						text={t('uncategorized.no_categories')}
						href="/rules#new-category"
						action={t('uncategorized.create_categories')}
					/>
				{/if}
				<!-- One box the height follows: on the list itself, it was gone with an empty filter and the end card came over
				     the row (pass 40). -->
				<div class="follows" bind:this={queue} use:followContent={holding}>
					<div class="stack">
						{#if shown.length > 0}
							<ul class="queue card" aria-label={t('uncategorized.list')}>
								{#each shown as transaction (transaction.id)}
									<li class:sorted={sorted?.id === transaction.id} out:list.leave animate:list.move>
										<Row
											{transaction}
											{categories}
											took={sorted?.id === transaction.id ? sorted.name : null}
											busy={busy === transaction.id}
											locked={busy !== null && busy !== transaction.id}
											onchoose={(id, select) => choose(transaction, id, select)}
											onundo={() => undo(transaction)}
										/>
										{#if sorted?.id === transaction.id && sorted.offering}
											<div class="more" transition:open onoutroend={follow}>
												<Offer
													description={transaction.description}
													category={sorted.category}
													{categories}
													{accounts}
													onclose={() => close(transaction.id)}
													onapplied={applied}
												/>
											</div>
										{/if}
									</li>
								{/each}
							</ul>
						{/if}
						{#if view.kind !== 'all' && counts[view.kind] === 0}
							<p class="hint" transition:open>{t('uncategorized.none_here')}</p>
						{/if}
					</div>
				</div>
				{#if waiting.length === 0}
					<div id="all-done" in:rise|global>
						<EmptyState
							icon="check"
							text={t('uncategorized.all_done')}
							href="/"
							action={t('uncategorized.to_summary')}
						/>
					</div>
				{/if}
			</div>
		{/if}
	</div>
{:else}
	<p class="hint loading" out:placeholder>{t('app.loading')}</p>
{/if}

<style>
	/* Measured from, for the list's height (followContent); no clip, so the list's shadow shows whole. */
	.follows {
		position: relative;
	}
	/* One list with a line between its rows. The columns live here, so every row's date, description, amount and
	   control share their edges; from 900 px a row is one line of about 52 px. */
	.queue {
		list-style: none;
		margin: 0;
		padding: 0;
		gap: 0;
		grid-template-columns: auto minmax(0, 1fr) auto minmax(180px, 240px);
		column-gap: var(--space-md);
	}
	.queue > li {
		display: grid;
		grid-column: 1 / -1;
		grid-template-columns: subgrid;
		/* Leaving, it takes the columns in numbers (`vanishRow`), and the gaps must stay with them. */
		column-gap: inherit;
		transition: background-color var(--duration-short) var(--ease-standard);
	}
	.queue > li + li {
		border-top: 1px solid var(--border);
	}
	.queue > li:first-child {
		border-top-left-radius: inherit;
		border-top-right-radius: inherit;
	}
	.queue > li:last-child {
		border-bottom-left-radius: inherit;
		border-bottom-right-radius: inherit;
	}
	.queue > li.sorted {
		background: color-mix(in oklab, var(--accent) 10%, var(--surface));
	}
	.more {
		grid-column: 1 / -1;
		padding: var(--space-xs) var(--space-md) var(--space-md);
	}
	@media (max-width: 899px) {
		.queue {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
