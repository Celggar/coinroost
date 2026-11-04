<script lang="ts">
	// A form opened on demand, on a phone: a full-screen dialog (Material 3) with its close button, its title and the
	// send button at the top, so the keyboard never covers them. It slides up and back down; with reduced motion it only
	// fades. A native `<dialog>`: the rest of the page is inert while it is open and Esc closes it.
	import { onMount, type Snippet } from 'svelte';
	import { prefersReducedMotion } from 'svelte/motion';
	import { fade, fly } from 'svelte/transition';
	import { t } from '#lib/i18n.svelte.ts';
	import { DURATION, EASE } from '#lib/motion.ts';

	let {
		title,
		form,
		action,
		onclose,
		children
	}: {
		title: string;
		/** The id of the form the top button sends. */
		form: string;
		action: string;
		onclose: () => void;
		children: Snippet;
	} = $props();

	let dialog = $state<HTMLDialogElement>();
	onMount(() => dialog?.showModal());

	// Modal to its last frame, it would keep the page inert while it slides away, so the focus could only go back once
	// it had gone, dropping to the page meanwhile. Shown again as a plain dialog in the same task, nothing flickers.
	function letGo() {
		dialog?.close();
		dialog?.show();
	}

	const slide = (node: Element, { out = false } = {}) =>
		prefersReducedMotion.current
			? fade(node, { duration: DURATION.short })
			: fly(node, {
					y: innerHeight,
					opacity: 1,
					duration: out ? DURATION.exit : DURATION.medium,
					easing: out ? EASE.exit : EASE.enter
				});
</script>

<dialog
	bind:this={dialog}
	aria-label={title}
	oncancel={(event) => {
		event.preventDefault();
		onclose();
	}}
	in:slide
	out:slide={{ out: true }}
	onoutrostart={letGo}
>
	<header>
		<button class="close" type="button" aria-label={t('app.close')} onclick={onclose}>
			<svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true">
				<path d="M6 6l12 12M18 6L6 18" />
			</svg>
		</button>
		<h2>{title}</h2>
		<button type="submit" {form}>{action}</button>
	</header>
	<div class="body">{@render children()}</div>
</dialog>

<style>
	dialog {
		position: fixed;
		inset: 0;
		width: 100%;
		max-width: none;
		height: 100dvh;
		max-height: none;
		margin: 0;
		padding: 0;
		border: 0;
		background: var(--surface);
		color: var(--text);
		overflow: auto;
		/* Leaving, it is a plain dialog (`letGo`): it still slides away over the bars and the menus. */
		z-index: 40;
	}
	dialog::backdrop {
		background: transparent;
	}
	header {
		position: sticky;
		top: 0;
		z-index: 1;
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: center;
		gap: var(--space-sm);
		min-height: 64px;
		padding: var(--space-xs) var(--space-md) var(--space-xs) var(--space-xs);
		border-bottom: 1px solid var(--border);
		background: var(--surface);
	}
	h2 {
		margin: 0;
	}
	.close {
		--state-layer: color-mix(in oklab, var(--accent) 10%, transparent);
		min-width: 48px;
		min-height: 48px;
		padding: 0;
		border: 0;
		background: transparent;
		color: var(--text);
	}
	svg {
		fill: none;
		stroke: currentColor;
		stroke-width: 2;
		stroke-linecap: round;
	}
	.body {
		padding: var(--space-lg);
		padding-bottom: calc(var(--space-lg) + env(safe-area-inset-bottom));
	}
</style>
