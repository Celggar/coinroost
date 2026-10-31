<script lang="ts">
	// The viewport notice (FD8, J12). It never covers an action: on a wide screen it sits bottom-right, away from the
	// buttons, which line up on the left; on a phone it drops in under the top bar (`offset`, its height), clear of the navigation bar at the bottom. Taps go
	// through it. Screen readers hear it from two regions that stay on the page (a region that arrives already filled
	// is often not announced): a polite one for what an action did, an assertive one for why it failed. It clears on
	// the next action: a press anywhere, a key that is not moving the focus, or a page change; the action that made
	// it is over by the time it shows.
	import { afterNavigate } from '$app/navigation';
	import { lift, rise } from '#lib/motion.ts';
	import { dismiss, notices } from './notices.svelte.ts';

	let { offset = 0 }: { offset?: number } = $props();

	const SETTLE_MS = 400;
	const phone = () => matchMedia('(max-width: 560px)').matches;

	function next(event: Event) {
		const shown = notices.current;
		if (!shown || performance.now() - shown.at < SETTLE_MS) return;
		// Moving the focus is not an action.
		if (event instanceof KeyboardEvent && (event.key === 'Tab' || event.key === 'Shift')) return;
		dismiss();
	}

	// It comes from the edge it sits on.
	const enter = (node: Element) => rise(node, { y: phone() ? -12 : 12 });
	const leave = (node: Element) => lift(node, { y: phone() ? -8 : 8 });

	afterNavigate(dismiss);
</script>

<svelte:document onpointerdown={next} onkeydown={next} />

<p class="visually-hidden" role="status">
	{notices.current?.tone === 'ok' ? notices.current.text : ''}
</p>
<p class="visually-hidden" role="alert">
	{notices.current?.tone === 'bad' ? notices.current.text : ''}
</p>

<div class="notices" aria-hidden="true" style:--bar="{offset}px">
	{#key notices.current?.id}
		{#if notices.current}
			<p class="note {notices.current.tone}" in:enter|global out:leave|global>
				<span class="mark"></span>
				<span class="text">{notices.current.text}</span>
			</p>
		{/if}
	{/key}
</div>

<style>
	.notices {
		position: fixed;
		z-index: 10;
		right: var(--space-xl);
		bottom: var(--space-xl);
		width: min(420px, calc(100vw - 2 * var(--space-xl)));
		display: grid;
		pointer-events: none;
	}
	.notices > :global(*) {
		grid-area: 1 / 1;
	}
	.note {
		margin: 0;
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding: var(--space-md) var(--space-lg);
		border-radius: var(--radius-button);
		background: var(--inverse-bg);
		color: var(--inverse-text);
		box-shadow: var(--card-shadow);
		overflow-wrap: anywhere;
	}
	.mark {
		flex: none;
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--inverse-ok);
	}
	.bad .mark {
		background: var(--inverse-bad);
	}
	.text {
		flex: 1;
	}
	@media (max-width: 899px) {
		.notices {
			top: calc(var(--bar, 0px) + var(--space-sm));
			bottom: auto;
			left: var(--space-lg);
			right: var(--space-lg);
			width: auto;
		}
	}
</style>
