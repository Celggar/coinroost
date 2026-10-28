<script lang="ts">
	// Spending per category or payee, most first, as bars on one scale.
	import { money } from '#lib/format.ts';
	import { plural, t } from '#lib/i18n.svelte.ts';

	export type Ranked = { key: string; name: string; spending: number; count: number };

	let {
		title,
		hint = '',
		items,
		currency,
		shown = 8
	}: { title: string; hint?: string; items: Ranked[]; currency: string; shown?: number } = $props();

	const id = $props.id();
	const most = $derived(Math.max(1, ...items.map((i) => i.spending)));
	const first = $derived(items.slice(0, shown));
	const rest = $derived(items.slice(shown));
</script>

{#snippet row(item: Ranked)}
	<li>
		<div class="line">
			<span class="name">{item.name}</span>
			<span class="amount">{money(item.spending, currency)}</span>
		</div>
		<div class="track" aria-hidden="true">
			<div class="fill" style:width="{(item.spending / most) * 100}%"></div>
		</div>
		<div class="hint">{plural(item.count, 'ranking.count_one', 'ranking.count_other')}</div>
	</li>
{/snippet}

<section class="card" aria-labelledby="{id}-title">
	<h2 id="{id}-title">{title}</h2>
	{#if hint}<p class="hint">{hint}</p>{/if}
	{#if items.length === 0}
		<p>{t('ranking.none')}</p>
	{:else}
		<ol class="ranking" aria-label={title}>
			{#each first as item (item.key)}{@render row(item)}{/each}
		</ol>
		{#if rest.length}
			<details>
				<summary>{t('ranking.more', { count: rest.length })}</summary>
				<ol class="ranking" start={shown + 1}>
					{#each rest as item (item.key)}{@render row(item)}{/each}
				</ol>
			</details>
		{/if}
	{/if}
</section>

<style>
	.ranking {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 10px;
	}
	.line {
		display: flex;
		justify-content: space-between;
		gap: 12px;
	}
	.name {
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.amount {
		font-variant-numeric: tabular-nums;
		font-weight: 600;
		white-space: nowrap;
	}
	.track {
		height: 8px;
		margin: 2px 0;
	}
	.fill {
		height: 100%;
		min-width: 2px;
		border-radius: 0 4px 4px 0;
		background: var(--chart-spending);
	}
	.hint {
		font-size: 0.85rem;
	}
	summary {
		cursor: pointer;
		padding: 8px 0;
		min-height: 44px;
	}
	details .ranking {
		margin-top: 4px;
	}
</style>
