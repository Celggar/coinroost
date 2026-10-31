<script lang="ts">
	// A page with nothing to show yet says what is missing and offers the one way on, as a card like the import page's
	// zone: the same look on every page that can be empty (summary, uncategorized, coverage, import).
	import Icon from '#lib/nav/Icon.svelte';
	import type { IconName } from '#lib/nav/destinations.ts';

	let { icon, text, href, action }: { icon: IconName; text: string; href: string; action: string } =
		$props();
</script>

<div class="card empty">
	<span class="icon"><Icon name={icon} size={28} /></span>
	<p>{text}</p>
	<a class="button" {href}>{action}</a>
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
