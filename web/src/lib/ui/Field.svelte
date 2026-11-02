<script lang="ts">
	// One field (FD4): the label, the control, then its hint and its error under it. The control comes in as a
	// snippet that receives the attributes tying it to them (`id`, `aria-describedby`, `aria-invalid`); `name` is the
	// key its error is kept under. The hint is text, or a snippet when it holds a link. The error opens under the
	// control and closes again (FD9); a hint that comes and goes fades, and one that replaces another fades through.
	import { untrack, type Snippet } from 'svelte';
	import { fade, slide } from 'svelte/transition';
	import { prefersReducedMotion } from 'svelte/motion';
	import { DURATION, EASE, fadeThrough } from '#lib/motion.ts';

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
	// Under reduced motion the error fades in place instead of opening.
	const open = (node: Element) =>
		prefersReducedMotion.current
			? fade(node, { duration: DURATION.short })
			: slide(node, { duration: DURATION.short, easing: EASE.enter });
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
	{#if error}<p class="field-error" id={errorId} transition:open>{error}</p>{/if}
</div>
