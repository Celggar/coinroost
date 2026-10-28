<script lang="ts">
	import type { Schemas } from '#lib/api/client.ts';
	import { money } from '#lib/format.ts';
	import { t } from '#lib/i18n.svelte.ts';

	let { totals }: { totals: Schemas['Totals'] } = $props();

	const net = $derived(totals.income - totals.spending);
	const unpaired = $derived(totals.unpaired_transfer_out + totals.unpaired_transfer_in);
</script>

<dl class="tiles">
	<div class="tile">
		<dt>{t('tiles.spending')}</dt>
		<dd class="value">{money(totals.spending, totals.currency)}</dd>
	</div>
	<div class="tile">
		<dt>{t('tiles.income')}</dt>
		<dd class="value">{money(totals.income, totals.currency)}</dd>
	</div>
	<div class="tile">
		<dt>{t('tiles.net')}</dt>
		<dd class="value">{money(net, totals.currency)}</dd>
	</div>
	<div class="tile">
		<dt>{t('tiles.unpaired')}</dt>
		<dd>
			{t('tiles.unpaired_detail', {
				out: money(totals.unpaired_transfer_out, totals.currency),
				in: money(totals.unpaired_transfer_in, totals.currency)
			})}
		</dd>
		{#if unpaired > 0}
			<dd><a class="target" href="/transfers">{t('tiles.see_transfers')}</a></dd>
		{/if}
	</div>
</dl>

<style>
	.tiles {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 160px), 1fr));
		gap: 8px;
		margin: 0;
	}
	.tile {
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		padding: 10px 12px;
	}
	dt {
		color: var(--muted);
		font-size: 0.9rem;
	}
	dd {
		margin: 0;
	}
	.value {
		font-size: 1.5rem;
		font-weight: 650;
	}
	.target {
		display: inline-block;
		padding: 4px 0;
		min-height: 24px;
	}
</style>
