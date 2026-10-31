<script lang="ts">
	// The wide screen's navigation (P1, N4): an expanded rail on the left (Material 3) with the login's mark, the
	// five destinations with a neutral pill on the current one, and Security and the account's button (P2) pinned to
	// its foot.
	import { t } from '#lib/i18n.svelte.ts';
	import AccountMenu from './AccountMenu.svelte';
	import Badge from './Badge.svelte';
	import Icon from './Icon.svelte';
	import Mark from './Mark.svelte';
	import NavCounts from './NavCounts.svelte';
	import { counts } from './counts.svelte.ts';
	import { isCurrent, wide } from './destinations.ts';

	let { path, onleave }: { path: string; onleave: () => void } = $props();

	const count = (badge: 'review' | 'accounts' | undefined) => (badge ? counts[badge] : 0);
</script>

<aside class="rail">
	<a class="brand" href="/"><Mark />{t('app.name')}</a>
	<nav aria-label={t('nav.main')}>
		<NavCounts id="rail" />
		<ul>
			{#each wide as destination (destination.href)}
				<li>
					<a
						href={destination.href}
						aria-current={isCurrent(destination, path) ? 'page' : undefined}
						aria-describedby={count(destination.badge) ? `rail-${destination.badge}` : undefined}
					>
						<span class="pill" aria-hidden="true"></span>
						<Icon name={destination.icon} />
						<span class="label">{t(destination.label)}</span>
						{#if destination.badge}
							<Badge
								count={count(destination.badge)}
								tone={destination.badge === 'accounts' ? 'warn' : 'bad'}
							/>
						{/if}
					</a>
				</li>
			{/each}
		</ul>
	</nav>
	<div class="foot">
		<a href="/security" aria-current={path === '/security' ? 'page' : undefined}>
			<span class="pill" aria-hidden="true"></span>
			<Icon name="shield" />
			<span class="label">{t('nav.security')}</span>
		</a>
		<AccountMenu {onleave} />
	</div>
</aside>

<style>
	/* Nothing scrolls under the rail, so it is a plain surface (the layout's column draws it); the bars on a phone,
	   which content scrolls under, take the login's glass. */
	/* Fixed, not sticky: at the end of a page whose height is not a whole pixel, a sticky rail met its column's end
	   and moved up 0.4 px (pass 6). It fills its column, inside the column's 1 px edge. */
	.rail {
		position: fixed;
		top: 0;
		left: 0;
		width: calc(var(--rail-width, 248px) - 1px);
		height: 100dvh;
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		padding: var(--space-lg) var(--space-md);
		overflow-y: auto;
	}
	.brand {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		min-height: 44px;
		padding: 0 var(--space-md);
		font-weight: 700;
		font-size: var(--text-lg);
		color: var(--text);
		text-decoration: none;
	}
	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: var(--space-xs);
	}
	a:not(.brand) {
		position: relative;
		isolation: isolate;
		display: flex;
		align-items: center;
		gap: var(--space-md);
		width: 100%;
		min-height: 48px;
		padding: 0 var(--space-lg);
		border-radius: 999px;
		color: var(--muted);
		font-weight: 600;
		text-decoration: none;
	}
	.label {
		flex: 1;
		text-align: start;
	}
	@media (hover: hover) {
		a:not(.brand):hover {
			color: var(--text);
		}
	}
	a[aria-current='page'] {
		color: var(--text);
	}
	/* The current item's pill grows from its centre (M3 active indicator); with reduced motion it only fades. */
	.pill {
		position: absolute;
		inset: 0;
		z-index: -1;
		border-radius: inherit;
		background: color-mix(in oklab, var(--accent) 14%, transparent);
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
		a[aria-current='page'] {
			outline: 2px solid CanvasText;
		}
	}
	.foot {
		margin-top: auto;
		display: grid;
		gap: var(--space-sm);
	}
</style>
