<script lang="ts">
	// One alert region that stays on the page, so screen readers announce what appears in it (a region that arrives
	// already filled is often not announced), with the message rising in and lifting out. A new `n` brings the same
	// message in again. The transitions are global because the message comes and goes with its `{#key}` block. The
	// card's height follows the message (the shell's job) and what sits below it slides along; the message rises
	// once that room is open, so the button never covers it and the card's edge never cuts it.
	import { prefersReducedMotion } from 'svelte/motion';
	import { DURATION, lift, rise } from '#lib/motion.ts';

	let { text, n }: { text: string; n: number } = $props();

	// The message lands under the field that keeps the focus; on a short screen it and the button after it could sit
	// below the fold (setup at 390 × 844: the message at 763–859). Once it is in, the page moves just enough to show
	// both: the form's send button when there is one, else the message.
	function reveal(node: HTMLElement) {
		const end = node.closest('form')?.querySelector<HTMLElement>('button[type="submit"]') ?? node;
		end.scrollIntoView({
			block: 'nearest',
			behavior: prefersReducedMotion.current ? 'auto' : 'smooth'
		});
	}
</script>

<div role="alert">
	{#key n}
		{#if text}<p
				class="alert"
				in:rise|global={{ delay: DURATION.medium }}
				out:lift|global
				onintroend={(event) => reveal(event.currentTarget)}
			>
				{text}
			</p>{/if}
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
