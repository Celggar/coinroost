<script lang="ts">
	// One field (FD4): the label, the control, then its hint and its error under it. The control comes in as a
	// snippet that receives the attributes tying it to them (`id`, `aria-describedby`, `aria-invalid`); `name` is the
	// key its error is kept under. The error opens under the control and closes again (FD9).
	import type { Snippet } from 'svelte';
	import { fade, slide } from 'svelte/transition';
	import { prefersReducedMotion } from 'svelte/motion';
	import { DURATION, EASE } from '#lib/motion.ts';

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
		hint?: string;
		error?: string;
		children: Snippet<[Wiring]>;
	} = $props();

	const id = $props.id();
	const hintId = `${id}-hint`;
	const errorId = `${id}-error`;
	const wiring = $derived<Wiring>({
		id,
		name,
		'aria-describedby': [hint && hintId, error && errorId].filter(Boolean).join(' ') || undefined,
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
	{#if hint}<p class="hint" id={hintId}>{hint}</p>{/if}
	{#if error}<p class="field-error" id={errorId} transition:open>{error}</p>{/if}
</div>
