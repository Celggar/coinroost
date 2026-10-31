<script lang="ts">
	// Six digit slots drawn over one real input, so pasting and the one-time-code autofill land in a single field;
	// the slots are decoration and hidden from screen readers. The field keeps digits only, six at most (a pasted
	// "123 456" becomes "123456"); the server takes spaces and dashes too. A digit pops in and fades out with its
	// `{#key}` block, hence the global transitions; the ring moves from slot to slot by fading. The sign-in pages keep
	// the hint above the slots (their approved design); a form in the app puts it under them and its error under that,
	// like every other field (FD4, FD5).
	import { fade, slide } from 'svelte/transition';
	import { prefersReducedMotion } from 'svelte/motion';
	import { DURATION, EASE, pop, shake as shakeRow, vanish } from '#lib/motion.ts';

	let {
		label,
		hint,
		value = $bindable(''),
		hintBelow = false,
		error = ''
	}: {
		label: string;
		hint: string;
		value?: string;
		hintBelow?: boolean;
		error?: string;
	} = $props();

	const id = $props.id();
	let focused = $state(false);
	let row: HTMLElement | undefined = $state();

	const described = $derived(error ? `${id}-hint ${id}-error` : `${id}-hint`);
	const open = (node: Element) =>
		prefersReducedMotion.current
			? fade(node, { duration: DURATION.short })
			: slide(node, { duration: DURATION.short, easing: EASE.enter });
	const digits = $derived(Array.from({ length: 6 }, (_, i) => value[i] ?? ''));
	const active = $derived(focused ? Math.min(value.length, 5) : -1);

	function clean(event: Event & { currentTarget: HTMLInputElement }) {
		const digitsOnly = event.currentTarget.value.replace(/\D/g, '').slice(0, 6);
		event.currentTarget.value = digitsOnly;
		value = digitsOnly;
	}

	let field: HTMLInputElement | undefined = $state();

	/** Puts the caret in the field, for a page that just swapped it in. */
	export function focus() {
		field?.focus();
	}

	/** Marks the code as wrong (the slots turn red until it is edited) and puts the caret back in it. */
	export function mark() {
		field?.setAttribute('aria-invalid', 'true');
		field?.focus();
	}

	/** Shakes the slots, next to the message that says the code was wrong. */
	export function shake() {
		shakeRow(row);
	}
</script>

<div class="code">
	<label for={id}>{label}</label>
	{#if !hintBelow}<span class="hint" id="{id}-hint">{hint}</span>{/if}
	<div class="row" bind:this={row}>
		{#each digits as digit, i (i)}
			{#if i === 3}<span class="dash" aria-hidden="true"></span>{/if}
			<span class="slot" class:active={i === active} aria-hidden="true">
				{#key digit}
					{#if digit}<span in:pop|global out:vanish|global>{digit}</span>{/if}
				{/key}
				{#if i === active && !digit}<span class="caret"></span>{/if}
			</span>
		{/each}
		<input
			bind:this={field}
			{id}
			type="text"
			inputmode="numeric"
			autocomplete="one-time-code"
			aria-describedby={described}
			aria-invalid={error ? 'true' : undefined}
			required
			spellcheck="false"
			{value}
			oninput={clean}
			onfocus={() => (focused = true)}
			onblur={() => (focused = false)}
		/>
	</div>
	{#if hintBelow}<span class="hint" id="{id}-hint">{hint}</span>{/if}
	{#if error}<p class="field-error" id="{id}-error" transition:open>{error}</p>{/if}
</div>

<style>
	/* As wide as six slots need: in a wide card the slots stay together instead of spreading apart. */
	.code {
		display: grid;
		gap: 6px;
		max-width: 360px;
	}
	.field-error {
		margin: 0;
	}
	.row {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 6px;
		margin-top: 2px;
	}
	.slot {
		position: relative;
		flex: 1 1 0;
		min-width: 0;
		max-width: 48px;
		height: 52px;
		display: grid;
		place-items: center;
		border: 1px solid var(--field-border);
		border-radius: 10px;
		background: var(--surface);
		font-size: 1.375rem;
		font-weight: 600;
		font-variant-numeric: tabular-nums;
	}
	.slot > :global(*) {
		grid-area: 1 / 1;
	}
	/* The real field is hidden, so its error shows on the slots, with the border every other field takes. */
	.row:has(input[aria-invalid='true']) .slot {
		border-color: var(--bad);
		box-shadow: 0 0 0 1px var(--bad);
	}
	/* The ring of the active slot is a layer of its own, so only its opacity changes. */
	.slot::after {
		content: '';
		position: absolute;
		inset: -1px;
		border: 1px solid var(--focus);
		border-radius: 10px;
		box-shadow:
			0 0 0 2px var(--ring-gap, var(--surface)),
			0 0 0 4px var(--focus);
		opacity: 0;
		pointer-events: none;
		transition: opacity var(--duration-short) var(--ease-standard);
	}
	.slot.active::after {
		opacity: 1;
	}
	/* Forced colors drop shadows: the system draws the ring as an outline. */
	@media (forced-colors: active) {
		.slot::after {
			display: none;
		}
		.slot.active {
			outline: 2px solid Highlight;
			outline-offset: 2px;
		}
	}
	/* A border, not a background: forced colors keep borders and drop backgrounds. */
	.dash {
		flex: none;
		width: 12px;
		height: 0;
		border-top: 2px solid var(--field-border);
	}
	/* A border, like the dash: a background would vanish in forced colors. */
	.caret {
		width: 0;
		height: 24px;
		border-left: 2px solid var(--text);
	}
	@media (prefers-reduced-motion: no-preference) {
		.caret {
			animation: blink 1s steps(1) infinite;
		}
	}
	@keyframes blink {
		50% {
			opacity: 0;
		}
	}
	/* The real field covers the slots: taps, paste and autofill go to it. 16 px keeps iOS from zooming in. */
	input {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		min-height: 0;
		margin: 0;
		padding: 0;
		border: 0;
		opacity: 0;
		font-size: 16px;
		cursor: text;
	}
</style>
