<script lang="ts">
	// One alert region that stays on the page, so screen readers announce what appears in it (a region that arrives
	// already filled is often not announced), with the message rising in and lifting out. A new `n` brings the same
	// message in again. The transitions are global because the message comes and goes with its `{#key}` block. The
	// card's height follows the message (the shell's job) and what sits below it slides along; the message rises
	// once that room is open, so the button never covers it and the card's edge never cuts it.
	import { prefersReducedMotion } from 'svelte/motion';
	import { DURATION, lift, rise } from '#lib/motion.ts';
	import { fadeAfter, fadePage } from '#lib/ui/followers.ts';
	import { showBelow } from '#lib/ui/landing.ts';

	let { text, n }: { text: string; n: number } = $props();

	let region: HTMLElement | undefined;
	let room = 0;

	const fade = () => {
		if (region?.closest('.glass')) fadePage(DURATION.medium);
		else if (region) fadeAfter(region, DURATION.medium);
	};

	// Where nothing may slide, the room stays once made: gone with the first key typed, it moved the centred card 30 px in
	// plain sight (pass 45). The room comes behind a fade, the whole card's when centred, begun before the message is in:
	// begun with it, the new place showed for a frame.
	$effect.pre(() => {
		if (text && !room && prefersReducedMotion.current) fade();
	});

	function makeRoom() {
		if (!region || !prefersReducedMotion.current) return;
		const height = region.offsetHeight;
		if (room && height - room >= 4) fade();
		room = Math.max(room, height);
		region.style.minHeight = `${room}px`;
	}

	// The message lands under the field that keeps the focus; on a short screen it and the button after it could sit
	// below the fold (setup at 390 × 844: the message at 763–859). Once it is in, the page moves just enough to show
	// both: the form's send button when there is one, else the message.
	function reveal(node: HTMLElement) {
		const end = node.closest('form')?.querySelector<HTMLElement>('button[type="submit"]') ?? node;
		const focused = document.activeElement;
		showBelow(end, focused instanceof HTMLElement && focused !== document.body ? focused : node);
	}
</script>

<div role="alert" bind:this={region}>
	{#key n}
		{#if text}<p
				class="alert"
				in:rise|global={{ delay: DURATION.medium }}
				out:lift|global
				onintrostart={makeRoom}
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
