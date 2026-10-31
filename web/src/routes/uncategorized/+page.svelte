<script lang="ts">
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import { refreshCounts } from '#lib/nav/counts.svelte.ts';
	import { onMount, tick } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { day } from '#lib/format.ts';
	import { plural, t } from '#lib/i18n.svelte.ts';
	import { glide, rise, vanish } from '#lib/motion.ts';
	import Money from '#lib/ui/Money.svelte';
	import Tick from '#lib/ui/Tick.svelte';
	import { notify } from '#lib/ui/notices.svelte.ts';

	let transactions = $state<Schemas['Transaction'][]>([]);
	let categories = $state<Schemas['Category'][]>([]);
	let loadError = $state('');
	let loaded = $state(false);
	// Nothing imported yet reads differently from everything sorted.
	let empty = $state(false);
	let busy = $state<string | null>(null);

	async function load() {
		loadError = '';
		const [list, kinds, any] = await Promise.all([
			api.GET('/api/v1/transactions', { params: { query: { uncategorized: true, limit: 1000 } } }),
			api.GET('/api/v1/categories'),
			api.GET('/api/v1/transactions', { params: { query: { limit: 1 } } })
		]);
		if (!list.data || !kinds.data) {
			const failed = list.data ? kinds : list;
			loadError = problem(failed.error, failed.response?.status, t('uncategorized.failed'));
			return;
		}
		transactions = list.data.items;
		categories = kinds.data.items;
		empty = any.data?.items.length === 0;
		loaded = true;
	}

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
		transactions = transactions.filter((x) => x.id !== transaction.id);
		const next = transactions[Math.min(at, transactions.length - 1)];
		await tick();
		if (next) document.getElementById(`category-${next.id}`)?.focus();
		else document.getElementById('all-done')?.focus();
	}

	onMount(load);
</script>

<svelte:head>
	<title>{t('app.title', { page: t('uncategorized.title') })}</title>
</svelte:head>

<h1>{t('uncategorized.title')}</h1>
<p class="hint">{t('uncategorized.hint')}</p>

{#if loadError}
	<p class="alert" role="alert">
		{loadError} <button class="secondary" onclick={load}>{t('app.try_again')}</button>
	</p>
{:else if loaded}
	<!-- The queue and its «all done» line take each other's place, the one fading over the other. -->
	<div class="in-place">
		{#if transactions.length === 0}
			{#if empty}
				<div in:rise>
					<EmptyState
						icon="import"
						text={t('uncategorized.none_yet')}
						href="/import"
						action={t('uncategorized.import_first')}
					/>
				</div>
			{:else}
				<p id="all-done" tabindex="-1" in:rise>{t('uncategorized.all_done')}</p>
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
				{#if categories.length === 0}
					<p class="hint">
						{t('uncategorized.no_categories')}
						<a href="/rules">{t('nav.rules')}</a>
					</p>
				{/if}
				<ul class="queue" aria-label={t('uncategorized.list')}>
					{#each transactions as transaction (transaction.id)}
						{@const leg = transaction.legs[0]}
						<li class="card" out:vanish animate:glide>
							<div class="txn-line">
								<span>{day(transaction.date)}</span>
								<span class="description">{transaction.description}</span>
								{#if leg}<Money minor={leg.amount_minor} currency={leg.currency} />{/if}
							</div>
							<label>
								{t('column.category')}
								<select
									id="category-{transaction.id}"
									value=""
									disabled={busy !== null || categories.length === 0}
									onchange={(event) => choose(transaction, event.currentTarget.value)}
								>
									<option value="" disabled>{t('uncategorized.choose')}</option>
									{#each categories as category (category.id)}
										<option value={category.id}>{category.name}</option>
									{/each}
								</select>
							</label>
						</li>
					{/each}
				</ul>
			</div>
		{/if}
	</div>
{:else}
	<p class="hint loading">{t('app.loading')}</p>
{/if}

<style>
	.queue {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: var(--space-md);
	}
</style>
