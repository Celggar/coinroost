<script lang="ts">
	// A page with nothing to show yet says what is missing and offers the one way on, as a card like the import page's
	// zone: the same look on every page that can be empty (summary, uncategorized, transfers, coverage, import).
	import Icon from '#lib/nav/Icon.svelte';
	import type { IconName } from '#lib/nav/destinations.ts';

	// The way on is a link, or a button where it is an action on this page (asking again with no network).
	let {
		icon,
		text,
		href,
		action,
		onclick
	}: { icon: IconName; text: string; action: string } & (
		{ href: string; onclick?: never } | { href?: never; onclick: () => void }
	) = $props();
</script>

<div class="card empty">
	<span class="icon"><Icon name={icon} size={28} /></span>
	<p>{text}</p>
	{#if onclick}
		<button class="button" type="button" {onclick}>{action}</button>
	{:else}
		<a class="button" {href}>{action}</a>
	{/if}
</div>

<style>
	.empty {
		display: grid;
		justify-items: center;
		gap: var(--space-md);
		padding: var(--space-2xl) var(--space-xl);
		text-align: center;
	}
	.empty p {
		max-width: 480px;
		text-wrap: balance;
	}
	.icon {
		display: grid;
		place-items: center;
		width: 56px;
		height: 56px;
		border-radius: 999px;
		background: color-mix(in oklab, var(--accent) 12%, var(--surface));
		color: var(--accent);
	}
</style>
