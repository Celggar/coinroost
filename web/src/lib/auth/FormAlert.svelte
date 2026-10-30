<script lang="ts">
	// One alert region that stays on the page, so screen readers announce what appears in it (a region that arrives
	// already filled is often not announced), with the message rising in and lifting out. A new `n` brings the same
	// message in again. The box grows and shrinks with its message instead of pushing the button in one jump.
	import { followHeight, lift, rise } from '#lib/motion.ts';

	let { text, n }: { text: string; n: number } = $props();
</script>

<div class="region" use:followHeight>
	<div role="alert">
		{#key n}
			{#if text}<p class="alert" in:rise out:lift>{text}</p>{/if}
		{/key}
	</div>
</div>

<style>
	.region {
		overflow: clip;
		height: 0;
	}
	[role='alert'] {
		display: grid;
	}
	[role='alert'] > :global(*) {
		grid-area: 1 / 1;
	}
	p {
		margin: 0 0 12px;
	}
	@media (prefers-reduced-motion: no-preference) {
		.region {
			transition: height var(--duration-medium) var(--ease-standard);
		}
	}
</style>
