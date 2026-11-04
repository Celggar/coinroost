<script lang="ts">
	// What the queue shows and in which order. The kinds are radios, so the arrow keys move between them; a count is of
	// the rows waiting, and changes in place.
	import { t } from '#lib/i18n.svelte.ts';
	import Tick from '#lib/ui/Tick.svelte';

	export type Kind = 'all' | 'out' | 'in';

	let {
		kind = $bindable(),
		byAmount = $bindable(),
		counts
	}: { kind: Kind; byAmount: boolean; counts: Record<Kind, number> } = $props();

	const kinds = $derived<{ value: Kind; label: string }[]>([
		{ value: 'all', label: t('uncategorized.all') },
		{ value: 'out', label: t('uncategorized.spending') },
		{ value: 'in', label: t('uncategorized.income') }
	]);
</script>

<div class="filters">
	<fieldset>
		<legend class="visually-hidden">{t('uncategorized.show')}</legend>
		{#each kinds as option (option.value)}
			<label class:chosen={kind === option.value}>
				<input type="radio" name="kind" value={option.value} bind:group={kind} />
				<span>{option.label}</span>
				<span aria-hidden="true">·</span>
				<Tick value={counts[option.value]}>{counts[option.value]}</Tick>
			</label>
		{/each}
	</fieldset>
	<button
		type="button"
		class="secondary"
		aria-pressed={byAmount}
		onclick={() => (byAmount = !byAmount)}>{t('uncategorized.by_amount')}</button
	>
</div>

<style>
	.filters {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-sm) var(--space-md);
	}
	fieldset {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-sm);
		margin: 0;
		padding: 0;
		border: 0;
		min-width: 0;
	}
	label {
		position: relative;
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		min-height: 44px;
		padding: 0 var(--space-md);
		border: 1px solid var(--field-border);
		border-radius: var(--radius-field);
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--link);
		cursor: pointer;
		transition:
			background-color var(--duration-short) var(--ease-standard),
			color var(--duration-short) var(--ease-standard);
	}
	label.chosen {
		color: var(--text);
		background: color-mix(in oklab, var(--accent) 22%, var(--surface));
	}
	/* The radio covers its pill: unseen, and what the keyboard and a press land on. */
	input {
		position: absolute;
		inset: 0;
		margin: 0;
		opacity: 0;
		cursor: pointer;
	}
	label:has(input:focus-visible) {
		outline: var(--focus-ring);
		outline-offset: 2px;
	}
	[aria-pressed='true'] {
		color: var(--text);
		background: color-mix(in oklab, var(--accent) 22%, var(--surface));
	}
	/* Under forced colors a fill does not show: the chosen ones take the system's mark. */
	@media (forced-colors: active) {
		label.chosen,
		[aria-pressed='true'] {
			background: Highlight;
			color: HighlightText;
			forced-color-adjust: none;
		}
	}
</style>
