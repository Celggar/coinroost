<script lang="ts">
	import type { Schemas } from '#lib/api/client.ts';
	import { money } from '#lib/format.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import Money from '#lib/ui/Money.svelte';
	import Tick from '#lib/ui/Tick.svelte';

	let { totals }: { totals: Schemas['Totals'] } = $props();

	const net = $derived(totals.income - totals.spending);
</script>

<div class="room">
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
		<!-- Like the others: its label, one figure (what left without its other side) with its word right after it, then
	     what arrived without one, in a line of its own. -->
		<div class="span">
			<dt>{t('tiles.unpaired')}</dt>
			<dd class="value">
				<Tick value={totals.unpaired_transfer_out}
					>{money(totals.unpaired_transfer_out, totals.currency)}</Tick
				>
				<span class="unit">{t('tiles.unpaired_out')}</span>
			</dd>
			<dd class="detail">
				<Tick value={totals.unpaired_transfer_in}
					>{t('tiles.unpaired_in', {
						in: money(totals.unpaired_transfer_in, totals.currency)
					})}</Tick
				>
			</dd>
		</div>
	</dl>
</div>

<style>
	.unit {
		font-size: var(--text-md);
		font-weight: 400;
		color: var(--muted);
	}
	.detail {
		color: var(--muted);
		font-size: var(--text-sm);
	}
	/* Four across where they fit, else two by two: never three and one left alone on a row (pass 2, 900 px). */
	.room {
		container-type: inline-size;
	}
	.tiles {
		grid-template-columns: repeat(4, minmax(0, 1fr));
	}
	@container (max-width: 700px) {
		.tiles {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}
	/* A phone: spending and income side by side, the net and the unpaired each across the width, so no tile is
	   stretched to a neighbour's height with an empty bottom. */
	@media (max-width: 560px) {
		.tiles {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
		.span {
			grid-column: 1 / -1;
		}
	}
</style>
