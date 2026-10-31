<script lang="ts">
	// A figure that changes in place (FD9, his #96): the old one lifts away as the new one rises into the same grid
	// cell, so a total never snaps to its new value. Under reduced motion both only fade. `value` is what changed.
	import type { Snippet } from 'svelte';
	import { fade, fly } from 'svelte/transition';
	import { prefersReducedMotion } from 'svelte/motion';
	import { DURATION, EASE } from '#lib/motion.ts';

	let { value, children }: { value: unknown; children: Snippet } = $props();

	const enter = (node: Element) =>
		prefersReducedMotion.current
			? fade(node, { duration: DURATION.short })
			: fly(node, { y: 8, duration: DURATION.medium, easing: EASE.enter });
	const leave = (node: Element) =>
		prefersReducedMotion.current
			? fade(node, { duration: DURATION.short })
			: fly(node, { y: -8, duration: DURATION.exit, easing: EASE.exit });
</script>

<span class="tick">
	{#key value}<span in:enter out:leave>{@render children()}</span>{/key}
</span>

<style>
	.tick {
		display: inline-grid;
	}
	.tick > span {
		grid-area: 1 / 1;
	}
</style>
