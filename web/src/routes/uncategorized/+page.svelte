<script lang="ts">
	import { focusOn, retry } from '#lib/ui/focus.ts';
	import { showWhole } from '#lib/ui/form.ts';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import { refreshCounts } from '#lib/nav/counts.svelte.ts';
	import { onMount, tick } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { day } from '#lib/format.ts';
	import { plural, t } from '#lib/i18n.svelte.ts';
	import { arrive, followContent, glide, placeholder, swapIn, vanish } from '#lib/motion.ts';
	import Money from '#lib/ui/Money.svelte';
	import Tick from '#lib/ui/Tick.svelte';
	import { notify } from '#lib/ui/notices.svelte.ts';

	let transactions = $state<Schemas['Transaction'][]>([]);
	let categories = $state<Schemas['Category'][]>([]);
	let accounts = $state<Schemas['Account'][]>([]);
	let loadError = $state('');
	let loaded = $state(false);
	// Nothing imported yet reads differently from everything sorted.
	let empty = $state(false);
	let busy = $state<string | null>(null);

	async function load() {
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
		transactions = list.data.items;
		categories = kinds.data.items;
		empty = any.data?.items.length === 0;
		accounts = held.data?.items ?? [];
		loaded = true;
	}

	let queue = $state<HTMLElement>();
	// Held while a sorted card leaves: the page glides once to the card that takes the focus, and the list gives back
	// the card's room after. Let go at once at the page's end, the page jumped 190 px in one frame (pass 15).
	let holding = $state(false);

	async function choose(transaction: Schemas['Transaction'], categoryId: string) {
		if (!categoryId) return;
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
			notify(problem(failure, response?.status, t('uncategorized.set_failed')), 'bad');
			return;
		}
		const name = categories.find((c) => c.id === categoryId)?.name;
		notify(t('uncategorized.done', { description: transaction.description, category: name ?? '' }));
		refreshCounts();
		// The focus moves on to the card that takes this one's place (or the one before it, at the end), so the
		// next choice is one key away and the focus never falls to the page.
		const at = transactions.findIndex((x) => x.id === transaction.id);
		holding = true;
		transactions = transactions.filter((x) => x.id !== transaction.id);
		const next = transactions[Math.min(at, transactions.length - 1)];
		await tick();
		// The page glides to it once the list has its new size: focused bare, the browser scrolls in one frame.
		const target = next ? focusOn(`#category-${next.id}`) : focusOn('#all-done a');
		const card = target?.closest('li');
		if (target) await showWhole(card ?? target, card ?? target, queue);
		holding = false;
	}

	onMount(load);
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
				<!-- Nothing imported yet: the step that comes first, as Summary and Import show it. Without an account
				     that reads a bank file, Import would only send people on to Accounts (pass 6). -->
				<div in:swapIn|global>
					{#if accounts.length === 0}
						<EmptyState
							icon="bank"
							text={t('dashboard.no_accounts')}
							href="/accounts"
							action={t('dashboard.to_accounts')}
						/>
					{:else if !accounts.some((a) => a.profile)}
						<EmptyState
							icon="bank"
							text={t('import.no_bank_file')}
							href="/accounts"
							action={t('file.open_accounts')}
						/>
					{:else}
						<EmptyState
							icon="import"
							text={t('uncategorized.none_yet')}
							href="/import"
							action={t('uncategorized.import_first')}
						/>
					{/if}
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
				<p>
					<Tick value={transactions.length}
						>{plural(
							transactions.length,
							'uncategorized.count_one',
							'uncategorized.count_other'
						)}</Tick
					>
				</p>
				<!-- With no category to choose, the way on comes first and the cards hold no control that cannot work. -->
				{#if categories.length === 0}
					<EmptyState
						icon="rules"
						text={t('uncategorized.no_categories')}
						href="/rules#new-category"
						action={t('uncategorized.create_categories')}
					/>
				{/if}
				<div class="follows" bind:this={queue} use:followContent={holding}>
					<ul class="queue txn-rows" aria-label={t('uncategorized.list')}>
						{#each transactions as transaction (transaction.id)}
							{@const leg = transaction.legs[0]}
							<li class="card" out:vanish animate:glide>
								<div class="txn-line">
									<span>{day(transaction.date)}</span>
									<span class="description">{transaction.description}</span>
									{#if leg}<Money minor={leg.amount_minor} currency={leg.currency} />{/if}
								</div>
								{#if categories.length > 0}<label>
										{t('column.category')}
										<select
											id="category-{transaction.id}"
											value=""
											disabled={busy !== null}
											onchange={(event) => choose(transaction, event.currentTarget.value)}
										>
											<option value="" disabled>{t('uncategorized.choose')}</option>
											{#each categories as category (category.id)}
												<option value={category.id}>{category.name}</option>
											{/each}
										</select>
									</label>{/if}
							</li>
						{/each}
					</ul>
				</div>
			</div>
		{/if}
	</div>
{:else}
	<p class="hint loading" out:placeholder>{t('app.loading')}</p>
{/if}

<style>
	/* Measured from, for the list's height (followContent); no clip, so the cards' shadows show whole. */
	.follows {
		position: relative;
	}
	.queue {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: var(--space-md);
	}
</style>
