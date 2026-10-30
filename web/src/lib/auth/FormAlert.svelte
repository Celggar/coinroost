<script lang="ts">
	// One alert region that stays on the page, so screen readers announce what appears in it (a region that arrives
	// already filled is often not announced), with the message rising in and lifting out. A new `n` brings the same
	// message in again. The transitions are global because the message comes and goes with its `{#key}` block. The
	// card's height follows the message (the shell's job), and what sits below it slides along.
	import { lift, rise } from '#lib/motion.ts';

	let { text, n }: { text: string; n: number } = $props();
</script>

<div role="alert">
	{#key n}
		{#if text}<p class="alert" in:rise|global out:lift|global>{text}</p>{/if}
	{/key}
</div>

<style>
	[role='alert'] {
		display: grid;
	}
	[role='alert'] > :global(*) {
		grid-area: 1 / 1;
	}
	p {
		margin: 0 0 12px;
	}
</style>
