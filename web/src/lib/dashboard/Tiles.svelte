<script lang="ts">
	import type { Schemas } from '#lib/api/client.ts';
	import { money } from '#lib/format.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import Money from '#lib/ui/Money.svelte';
	import Tick from '#lib/ui/Tick.svelte';

	let { totals }: { totals: Schemas['Totals'] } = $props();

	const net = $derived(totals.income - totals.spending);
</script>

<dl class="figures tiles">
	<div>
		<dt>{t('tiles.spending')}</dt>
		<dd class="value">
			<Tick value={totals.spending}>{money(totals.spending, totals.currency)}</Tick>
		</dd>
	</div>
	<div>
		<dt>{t('tiles.income')}</dt>
		<dd class="value">
			<Tick value={totals.income}>{money(totals.income, totals.currency)}</Tick>
		</dd>
	</div>
	<div class="span">
		<dt>{t('tiles.net')}</dt>
		<dd class="value">
			<Tick value={net}><Money minor={net} currency={totals.currency} /></Tick>
		</dd>
	</div>
</dl>

<style>
	/* Three across; on a phone spending and income side by side and the net across the width, so no tile is
	   stretched to a neighbour's height with an empty bottom. What once was a fourth tile, the transfers without a
	   pair, is a row of «Por revisar» (P3). */
	.tiles {
		grid-template-columns: repeat(3, minmax(0, 1fr));
	}
	@media (max-width: 560px) {
		.tiles {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
		.span {
			grid-column: 1 / -1;
		}
	}
	/* On the narrowest phones two figures side by side ran 2.8 px into their tiles' padding (pass 13, 320 px): one per
	   row, as the date ranges do. */
	@media (max-width: 359px) {
		.tiles {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
