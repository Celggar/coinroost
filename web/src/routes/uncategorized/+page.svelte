<script lang="ts">
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { money } from '#lib/format.ts';

	let transactions = $state<Schemas['Transaction'][]>([]);
	let categories = $state<Schemas['Category'][]>([]);
	let loadError = $state('');
	let error = $state('');
	let done = $state('');
	let loaded = $state(false);
	let busy = $state<string | null>(null);

	async function load() {
		loadError = '';
		const [list, kinds] = await Promise.all([
			api.GET('/api/v1/transactions', { params: { query: { uncategorized: true, limit: 1000 } } }),
			api.GET('/api/v1/categories')
		]);
		if (!list.data || !kinds.data) {
			const failed = list.data ? kinds : list;
			loadError = problem(
				failed.error,
				failed.response?.status,
				'The transactions could not be loaded'
			);
			return;
		}
		transactions = list.data.items;
		categories = kinds.data.items;
		loaded = true;
	}

	async function choose(transaction: Schemas['Transaction'], categoryId: string) {
		if (!categoryId) return;
		busy = transaction.id;
		error = '';
		done = '';
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
			error = problem(failure, response?.status, 'The category could not be set');
			return;
		}
		const name = categories.find((c) => c.id === categoryId)?.name;
		done = `${transaction.description}: ${name}.`;
		transactions = transactions.filter((t) => t.id !== transaction.id);
	}

	onMount(load);
</script>

<svelte:head>
	<title>Uncategorized · Coinroost</title>
</svelte:head>

<h1>Uncategorized</h1>
<p class="hint">
	Transactions no rule sorted. A category you choose here stays: rules never change it.
</p>

{#if loadError}
	<p class="alert" role="alert">
		{loadError} <button class="secondary" onclick={load}>Try again</button>
	</p>
{:else if loaded}
	<div class="stack">
		{#if error}<p class="alert" role="alert">{error}</p>{/if}
		{#if done}<p class="notice" role="status">{done}</p>{/if}
		{#if transactions.length === 0}
			<p>Every transaction has a category.</p>
		{:else}
			<p>{transactions.length} without a category.</p>
			{#if categories.length === 0}
				<p class="hint">Create categories on the <a href="/rules">Rules</a> page first.</p>
			{/if}
			<ul class="queue" aria-label="Transactions without a category">
				{#each transactions as transaction (transaction.id)}
					{@const leg = transaction.legs[0]}
					<li class="card">
						<div class="what">
							<span>{transaction.date}</span>
							<span class="description">{transaction.description}</span>
							{#if leg}<span class="amount">{money(leg.amount_minor, leg.currency)}</span>{/if}
						</div>
						<label>
							Category
							<select
								value=""
								disabled={busy !== null || categories.length === 0}
								onchange={(event) => choose(transaction, event.currentTarget.value)}
							>
								<option value="" disabled>Choose a category</option>
								{#each categories as category (category.id)}
									<option value={category.id}>{category.name}</option>
								{/each}
							</select>
						</label>
					</li>
				{/each}
			</ul>
		{/if}
	</div>
{:else}
	<p class="hint">Loading…</p>
{/if}

<style>
	.queue {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 8px;
	}
	.what {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 12px;
	}
	.description {
		flex: 1;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.amount {
		font-variant-numeric: tabular-nums;
	}
</style>
