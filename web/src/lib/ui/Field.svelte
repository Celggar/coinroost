<script lang="ts">
	// One field (FD4): the label, the control, then its hint and its error under it. The control comes in as a
	// snippet that receives the attributes tying it to them (`id`, `aria-describedby`, `aria-invalid`); `name` is the
	// key its error is kept under. The hint is text, or a snippet when it holds a link. The error opens under the
	// control and closes again (FD9); a hint that comes and goes fades, and one that replaces another fades through.
	import { untrack, type Snippet } from 'svelte';
	import { fade, slide } from 'svelte/transition';
	import { prefersReducedMotion } from 'svelte/motion';
	import { DURATION, EASE, fadeThrough } from '#lib/motion.ts';
	import { inView, showWhole } from '#lib/ui/landing.ts';

	export type Wiring = {
		id: string;
		name: string;
		'aria-describedby': string | undefined;
		'aria-invalid': 'true' | undefined;
	};

	let {
		label,
		name,
		hint = '',
		error = '',
		children
	}: {
		label: string;
		name: string;
		hint?: string | Snippet;
		error?: string;
		children: Snippet<[Wiring]>;
	} = $props();

	const id = $props.id();
	const hintId = `${id}-hint`;
	const errorId = `${id}-error`;
	// The hint on screen. One replacing another fades through on the same line: swapped in place, the import's
	// note that no account reads the file vanished in one frame when the error took its place (pass 11).
	let shown = $state(untrack(() => hint));
	let hintLine = $state<HTMLElement>();
	let swapping = Promise.resolve();
	$effect(() => {
		const next = hint;
		untrack(() => {
			if (next === shown) return;
			if (!shown || !next) {
				shown = next;
				return;
			}
			swapping = swapping.then(() => fadeThrough(hintLine, () => void (shown = hint)));
		});
	});
	const wiring = $derived<Wiring>({
		id,
		name,
		'aria-describedby': [shown && hintId, error && errorId].filter(Boolean).join(' ') || undefined,
		'aria-invalid': error ? 'true' : undefined
	});
	// The error opens and fades in together, at Material's standard pace: slide's own opacity reaches 1 in a twentieth
	// of the time, so the text showed whole in one frame with two thirds of its height (pass 12); the later `opacity`
	// wins. Under reduced motion it only fades in place.
	const open = (node: Element) => {
		if (prefersReducedMotion.current) return fade(node, { duration: DURATION.short });
		const opening = slide(node, { duration: DURATION.medium, easing: EASE.standard });
		return {
			...opening,
			css: (t: number, u: number) => `${opening.css?.(t, u) ?? ''}; opacity: ${t}`
		};
	};
	// Opened at a page's end, the error sat under the bottom bar while its field kept the focus above it (pass 21: a
	// file's account at 390 × 600). Once the page settles (a card grows to hold it), it moves just enough to show the
	// error, which sits right under its field. Only the focused field's: a form sent with several would start as many.
	function reveal(node: HTMLElement) {
		const own = document.activeElement
			?.getAttribute('aria-describedby')
			?.split(' ')
			.includes(node.id);
		// Already whole between the bars, it moves nothing: showWhole also clears lines cut by the top bar, and slid the
		// page 110 px under an error that showed mid-screen (pass 22, Reglas at 390).
		if (own && !inView(node)) showWhole(node);
	}
</script>

<div class="form-field">
	<label for={id}>{label}</label>
	{@render children(wiring)}
	{#if shown}<p
			class="hint"
			id={hintId}
			bind:this={hintLine}
			in:fade={{ duration: DURATION.short }}
			out:fade={{ duration: DURATION.short }}
		>
			{#if typeof shown === 'string'}{shown}{:else}{@render shown()}{/if}
		</p>{/if}
	{#if error}<p
			class="field-error"
			id={errorId}
			transition:open
			onintroend={(event) => reveal(event.currentTarget)}
		>
			{error}
		</p>{/if}
</div>
