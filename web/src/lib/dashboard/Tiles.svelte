<script lang="ts">
	import type { Schemas } from '#lib/api/client.ts';
	import { money } from '#lib/format.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import Money from '#lib/ui/Money.svelte';

	let { totals }: { totals: Schemas['Totals'] } = $props();

	const net = $derived(totals.income - totals.spending);
	const unpaired = $derived(totals.unpaired_transfer_out + totals.unpaired_transfer_in);
</script>

<dl class="figures">
	<div>
		<dt>{t('tiles.spending')}</dt>
		<dd class="value">{money(totals.spending, totals.currency)}</dd>
	</div>
	<div>
		<dt>{t('tiles.income')}</dt>
		<dd class="value">{money(totals.income, totals.currency)}</dd>
	</div>
	<div>
		<dt>{t('tiles.net')}</dt>
		<dd class="value"><Money minor={net} currency={totals.currency} /></dd>
	</div>
	<div>
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
	/* A link that is a tap target of its own: 44 px tall. */
	.target {
		display: inline-flex;
		align-items: center;
		min-height: 44px;
	}
</style>
