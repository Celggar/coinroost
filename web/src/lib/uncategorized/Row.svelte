<script lang="ts">
	// One transaction waiting for a category, or the last one sorted. A sorted row stays where it was, tinted, until the
	// next choice (no timer: WCAG 2.2.1), says what it took and offers Deshacer.
	import type { Schemas } from '#lib/api/client.ts';
	import { day } from '#lib/format.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import { swapIn, vanish } from '#lib/motion.ts';
	import Arrow from '#lib/ui/Arrow.svelte';
	import Money from '#lib/ui/Money.svelte';

	let {
		transaction,
		categories,
		took,
		busy,
		locked,
		onchoose,
		onundo
	}: {
		transaction: Schemas['Transaction'];
		categories: Schemas['Category'][];
		/** The name of the category it took, once sorted. */
		took: string | null;
		busy: boolean;
		/** Another row is being sent. */
		locked: boolean;
		onchoose: (id: string, select: HTMLSelectElement) => void;
		onundo: () => void;
	} = $props();

	const leg = $derived(transaction.legs[0]);
</script>

<div class="row" class:sorted={took}>
	<span class="date">{day(transaction.date)}</span>
	<span class="description">
		<span class="name">{transaction.description}</span>{#if took}<Arrow /><span class="took"
				>{took}</span
			>{/if}
	</span>
	{#if leg}<span class="amount"><Money minor={leg.amount_minor} currency={leg.currency} /></span
		>{/if}
	{#if categories.length > 0}
		<div class="control in-place">
			{#if took}
				<!-- Global: it comes with the row's change, not with a block of its own opening. -->
				<div in:swapIn|global out:vanish|global>
					<button
						id="undo-{transaction.id}"
						class="secondary"
						aria-disabled={busy}
						onclick={() => busy || onundo()}>{t('uncategorized.undo')}</button
					>
				</div>
			{:else}
				<div in:swapIn|global out:vanish|global>
					<label>
						<span class="visually-hidden">{t('column.category')}</span>
						<select
							id="category-{transaction.id}"
							value=""
							disabled={locked}
							aria-disabled={busy}
							onchange={(event) => onchoose(event.currentTarget.value, event.currentTarget)}
						>
							<option value="" disabled>{t('uncategorized.choose')}</option>
							{#each categories as category (category.id)}
								<option value={category.id}>{category.name}</option>
							{/each}
						</select>
					</label>
				</div>
			{/if}
		</div>
	{/if}
</div>

<style>
	/* The list holds the columns and each row takes them, so every description starts and every amount ends at one
	   edge. On a phone: description and amount, the date, then the control. */
	.row {
		display: grid;
		grid-column: 1 / -1;
		grid-template-columns: subgrid;
		align-items: center;
		padding: var(--space-xs) var(--space-md);
		transition: background-color var(--duration-short) var(--ease-standard);
	}
	.date {
		font-size: var(--text-sm);
		color: var(--muted);
		white-space: nowrap;
	}
	.description {
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.sorted .name {
		font-weight: 600;
	}
	.amount {
		justify-self: end;
		white-space: nowrap;
	}
	.control select,
	.control button {
		width: 100%;
	}
	@media (max-width: 899px) {
		.row {
			grid-template-columns: minmax(0, 1fr) auto;
			gap: var(--space-2xs) var(--space-md);
			padding: var(--space-md);
		}
		.description {
			grid-column: 1;
			grid-row: 1;
		}
		.amount {
			grid-column: 2;
			grid-row: 1;
			align-self: start;
		}
		.date {
			grid-column: 1 / -1;
			grid-row: 2;
		}
		.control {
			grid-column: 1 / -1;
			grid-row: 3;
			margin-top: var(--space-xs);
		}
	}
</style>
