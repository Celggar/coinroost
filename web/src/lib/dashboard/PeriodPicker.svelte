<script lang="ts">
	// The dashboard's period (P3): one segmented control (three presets and «Otro…», which alone shows the two dates)
	// and the range the figures stand for. `from` and `to` are the period asked for; `shown` is the one the figures
	// below show, and `stale` says they are dimmed while the next one loads.
	import { span } from '#lib/format.ts';
	import { fade, slide } from 'svelte/transition';
	import { prefersReducedMotion } from 'svelte/motion';
	import { preset, type Period, type Preset } from '#lib/dashboard/period.ts';
	import { t, type Key } from '#lib/i18n.svelte.ts';
	import { DURATION, EASE, pop, vanish } from '#lib/motion.ts';
	import DateRange from '#lib/ui/DateRange.svelte';
	import { fadeAfter } from '#lib/ui/followers.ts';

	let {
		from = $bindable(),
		to = $bindable(),
		shown,
		stale = false
	}: { from: string; to: string; shown: Period; stale?: boolean } = $props();

	// One segmented control for the period (P3): three presets and «Otro…», which alone shows the two dates.
	// Below 360 px English «This month» and «This year» left 5 px of air in their segments: there they read «Month»
	// and «Year», the range under the control saying which (P3 round 1).
	const presets: { which: Preset | 'other'; label: Key; short?: Key }[] = [
		{ which: 'month', label: 'dashboard.this_month', short: 'dashboard.this_month_short' },
		{ which: 'year', label: 'dashboard.this_year', short: 'dashboard.this_year_short' },
		{ which: 'twelve', label: 'dashboard.twelve' },
		{ which: 'other', label: 'dashboard.other' }
	];

	// «Otro…» chosen: the dates show and are the period, even when they happen to equal a preset.
	let custom = $state(false);

	function choose(which: Preset | 'other') {
		if (which === 'other') {
			custom = true;
			return;
		}
		custom = false;
		({ from, to } = preset(which));
	}

	function chosen(which: Preset | 'other'): boolean {
		if (which === 'other') return custom;
		const range = preset(which);
		return !custom && from === range.from && to === range.to;
	}

	// The dates open under the control and close again (FD9); under reduced motion they only fade. Their height moves
	// the figures under them, which begin and end on screen: Material's standard easing, whose last frame is short
	// (the exit easing ended on a 31 px step).
	const open = (node: Element) =>
		prefersReducedMotion.current
			? fade(node, { duration: DURATION.short })
			: slide(node, { duration: DURATION.medium, easing: EASE.standard });
	const close = (node: Element) =>
		prefersReducedMotion.current
			? fade(node, { duration: DURATION.short })
			: slide(node, { duration: DURATION.exit, easing: EASE.standard });
	// Under reduced motion their room comes and goes at once: what follows fades in at its new place.
	function follow(event: Event) {
		const period = (event.currentTarget as Element).parentElement;
		if (period && prefersReducedMotion.current) fadeAfter(period, DURATION.medium);
	}
</script>

<!-- The period (P3): one segmented control under its label and the range the figures stand for, beside it
 where the control has its full width, else under it; the two dates only under «Otro…». The range is
 always there, the period of the figures below (dimmed with them while the next loads), so nothing comes
 or goes when the dates open or close. -->
<div class="period">
	<div class="period-row">
		<div class="form-field" role="group" aria-labelledby="period-label">
			<span class="label" id="period-label">{t('dashboard.period')}</span>
			<div class="presets">
				{#each presets as option (option.which)}
					{@const on = chosen(option.which)}
					<button
						class="secondary"
						aria-pressed={on}
						aria-controls={option.which === 'other' && custom ? 'period-dates' : undefined}
						onclick={() => choose(option.which)}
						>{#if on}<svg class="check" viewBox="0 0 24 24" aria-hidden="true" in:pop out:vanish
								><path d="M5 12.5l4.5 4.5L19 7.5" /></svg
							>{/if}{#if option.short}<span class="long">{t(option.label)}</span><span class="short"
								>{t(option.short)}</span
							>{:else}{t(option.label)}{/if}</button
					>
				{/each}
			</div>
		</div>
		<p class="range in-place" class:stale>
			{#key `${shown.from}/${shown.to}`}<span
					in:fade={{ duration: DURATION.short, delay: DURATION.short }}
					out:fade={{ duration: DURATION.short }}>{span(shown.from, shown.to)}</span
				>{/key}
		</p>
	</div>
	<!-- The gap above the dates is their own padding, so it opens and closes with them. -->
	{#if custom}
		<div
			class="dates"
			id="period-dates"
			in:open
			out:close
			onintrostart={follow}
			onoutroend={follow}
		>
			<DateRange bind:from bind:to />
		</div>
	{/if}
</div>

<style>
	/* The period's width decides its layout (a container query, not the window's): the range goes beside the
	   control only where the control keeps its full 37rem, else under it with the control as wide as the cards; at
	   900 px the control stopped 28 px short of them (P3 round 1). */
	.period {
		container: period / inline-size;
	}
	.period-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-md);
	}
	.presets {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
	}
	/* The transition lives here, not on the dimmed state: there, taking the class away lit the range in one frame. */
	.range {
		margin: 0;
		color: var(--muted);
		transition: opacity var(--duration-exit) var(--ease-standard);
	}
	@container period (min-width: 54rem) {
		.period-row {
			grid-template-columns: minmax(0, 37rem) auto;
			align-items: end;
			column-gap: var(--space-lg);
		}
		/* Level with the control's middle, not its label: as tall as the control, its line centered. */
		.range {
			min-height: var(--field-height);
			align-items: center;
		}
	}
	.dates {
		padding-top: var(--space-lg);
	}
	.presets button {
		white-space: nowrap;
	}
	.presets button {
		min-height: var(--field-height);
		border-radius: 0;
		padding: 0 var(--space-lg);
	}
	/* Neighbours share one border line; the chosen one draws its own all round, inside. */
	.presets button + button {
		border-left-width: 0;
	}
	.presets button:first-child {
		border-radius: var(--radius-field) 0 0 var(--radius-field);
	}
	.presets button:last-child {
		border-radius: 0 var(--radius-field) var(--radius-field) 0;
	}
	.label {
		font-weight: 600;
		line-height: 1.25;
	}
	/* Material 3: the chosen segment is filled and carries a check; the outline stays the group's. */
	.presets button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-xs);
	}
	.presets button[aria-pressed='true'] {
		background: color-mix(in oklab, var(--accent) 22%, var(--surface));
		color: var(--text);
	}
	/* Forced colors drop the fill: the chosen segment takes the system's selection colors, as a selected item does,
	   so it shows on a phone too, where it has no check (P3 round 1). */
	@media (forced-colors: active) {
		.presets button[aria-pressed='true'] {
			forced-color-adjust: none;
			background: Highlight;
			color: HighlightText;
			border-color: Highlight;
		}
	}
	.check {
		flex: none;
		width: 18px;
		height: 18px;
		fill: none;
		stroke: currentColor;
		stroke-width: 2.5;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	/* A phone: the four segments across the width with a smaller label, the range under them. Each segment takes
	   its label's width plus an equal share of the rest (UIKit's segments «apportioned by content»): equal ones left
	   «This month» and «12 meses» touching their borders. The chosen one shows by its fill alone, as Material's
	   segmented button allows: the check needs room four segments do not have there. */
	@media (max-width: 560px) {
		.presets {
			display: flex;
		}
		.presets button {
			flex: 1 1 auto;
			padding: 0 var(--space-sm);
			font-size: var(--text-sm);
		}
		.check {
			display: none;
		}
	}
	.short {
		display: none;
	}
	@media (max-width: 359px) {
		.long {
			display: none;
		}
		.short {
			display: inline;
		}
		.presets button {
			padding: 0 var(--space-xs);
			font-size: var(--text-xs);
		}
	}
	/* While another period loads, the range dims with the figures. */
	.stale {
		opacity: 0.6;
	}
</style>
