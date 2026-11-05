<script lang="ts">
	// The phone's navigation (P1, N3): a 56 px top bar with the page's title, and a bar held to the bottom of the screen
	// (sticky, last in the page's column) with the five destinations as labelled icons (Material 3 navigation bar), always on screen and in reach of the thumb.
	import { page } from '$app/state';
	import { t } from '#lib/i18n.svelte.ts';
	import Badge from './Badge.svelte';
	import Icon from './Icon.svelte';
	import Mark from './Mark.svelte';
	import NavCounts from './NavCounts.svelte';
	import { counts } from './counts.svelte.ts';
	import { errorTitle, isCurrent, narrow, titleOf } from './destinations.ts';

	let {
		path,
		top = $bindable(0),
		bottom = $bindable(0)
	}: { path: string; top?: number; bottom?: number } = $props();

	const count = (badge: 'review' | 'accounts' | undefined) => (badge ? counts[badge] : 0);
	// The error page has no destination: the bar says what its heading says.
	const title = $derived(page.error ? errorTitle(page.status) : titleOf(path));
</script>

<header class="appbar" bind:offsetHeight={top}>
	{#if path === '/'}<Mark />{/if}
	<span class="title">{t(title)}</span>
</header>

<nav class="bar" aria-label={t('nav.main')} bind:offsetHeight={bottom}>
	<NavCounts id="bar" />
	<ul>
		{#each narrow as destination (destination.href)}
			<li>
				<a
					href={destination.href}
					aria-current={isCurrent(destination, path) ? 'page' : undefined}
					aria-describedby={count(destination.badge) ? `bar-${destination.badge}` : undefined}
				>
					<span class="indicator">
						<span class="pill" aria-hidden="true"></span>
						<Icon name={destination.icon} />
						{#if destination.badge}
							<span class="corner">
								<Badge
									count={count(destination.badge)}
									tone={destination.badge === 'accounts' ? 'warn' : 'bad'}
								/>
							</span>
						{/if}
					</span>
					<span class="label">{t(destination.label)}</span>
				</a>
			</li>
		{/each}
	</ul>
</nav>

<style>
	.appbar,
	.bar {
		background: color-mix(in oklab, var(--surface) 82%, transparent);
		-webkit-backdrop-filter: blur(16px) saturate(160%);
		backdrop-filter: blur(16px) saturate(160%);
	}
	@media (prefers-reduced-transparency: reduce), (prefers-contrast: more), (forced-colors: active) {
		.appbar,
		.bar {
			background: var(--surface);
			-webkit-backdrop-filter: none;
			backdrop-filter: none;
		}
	}
	.appbar {
		order: 1;
		position: sticky;
		top: 0;
		z-index: 5;
		display: flex;
		align-items: center;
		gap: var(--space-md);
		height: 56px;
		padding: 0 var(--space-lg);
		border-bottom: 1px solid var(--card-edge);
	}
	.title {
		font-size: var(--text-lg);
		font-weight: 700;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	/* On a phone the page's heading is out of sight and the bar's title says it: when the focus goes to the heading
	   (after a retry, or once an item it was on is gone), the title wears the ring, or nothing on screen showed
	   where the focus was (pass 9). */
	:global(body:has(#main h1:focus-visible)) .title {
		outline: 3px solid var(--focus);
		outline-offset: 2px;
		border-radius: 4px;
	}
	.bar {
		order: 3;
		position: sticky;
		z-index: 5;
		bottom: 0;
		padding: var(--space-xs) var(--space-xs)
			calc(var(--space-xs) + env(safe-area-inset-bottom, 0px));
		border-top: 1px solid var(--card-edge);
	}
	/* The bar is always on screen, but it stands inside the page's scroll padding, so a browser that focuses one of its
	   links takes it for hidden and moves the page to centre it: 395 px at each Tab on a phone (pass 35). */
	.bar a {
		scroll-margin-bottom: -100vh;
	}
	/* The narrowest phones: no side padding, so each of the five destinations keeps 64 px (N3). */
	@media (max-width: 359px) {
		.bar {
			padding-inline: 0;
		}
	}
	ul {
		list-style: none;
		margin: 0 auto;
		padding: 0;
		max-width: 560px;
		display: grid;
		grid-template-columns: repeat(5, minmax(0, 1fr));
	}
	a {
		display: grid;
		justify-items: center;
		align-content: center;
		gap: var(--space-xs);
		min-height: 64px;
		color: var(--muted);
		font-size: var(--text-xs);
		font-weight: 400;
		text-decoration: none;
		/* Its focus ring's corners, round as the rail's: square, it read as another control (pass 35). */
		border-radius: var(--radius-button);
	}
	@media (hover: hover) {
		a:hover {
			color: var(--text);
		}
	}
	/* The pill is a tint under 3:1 against the bar, so the weight and the icon say «you are here» too (N5, pass 29). */
	a[aria-current='page'] {
		color: var(--text);
		font-weight: 700;
	}
	a[aria-current='page'] :global(.icon) {
		fill: color-mix(in oklab, currentColor 22%, transparent);
		stroke-width: 2.6;
	}
	a[aria-current='page'] :global(.icon.dots) {
		stroke-width: 4;
	}
	.indicator {
		position: relative;
		isolation: isolate;
		display: grid;
		place-items: center;
		width: 64px;
		height: 32px;
		border-radius: 999px;
	}
	/* The current item's pill grows from its centre (M3 active indicator); with reduced motion it only fades. */
	.pill {
		position: absolute;
		inset: 0;
		z-index: -1;
		border-radius: inherit;
		background: color-mix(in oklab, var(--accent) 16%, transparent);
		opacity: 0;
		transform: scaleX(0.5);
		transition: opacity var(--duration-medium) var(--ease-standard);
	}
	a[aria-current='page'] .pill {
		opacity: 1;
		transform: none;
	}
	@media (prefers-reduced-motion: no-preference) {
		.pill {
			transition:
				opacity var(--duration-medium) var(--ease-standard),
				transform var(--duration-medium) var(--ease-standard);
		}
	}
	@media (forced-colors: active) {
		a[aria-current='page'] .indicator {
			outline: 2px solid CanvasText;
		}
	}
	.corner {
		position: absolute;
		top: -6px;
		left: calc(50% + 6px);
	}
	.label {
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	/* There the five targets fill the screen: the pill and the labels keep clear of their edges, so the outer ones do
	   not touch the screen's (pass 7); after the rules they narrow. */
	@media (max-width: 359px) {
		.indicator {
			width: 56px;
		}
		a {
			font-size: 0.75rem;
		}
	}
</style>
