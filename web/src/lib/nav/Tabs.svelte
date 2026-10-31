<script lang="ts">
	// The two pages under Revisar and under Cuentas (P1, N2), as tabs at the top of each: links, the current one
	// marked, with an underline that slides in.
	import { t } from '#lib/i18n.svelte.ts';
	import Badge from './Badge.svelte';
	import { counts } from './counts.svelte.ts';
	import { groups, type Tab } from './destinations.ts';

	let { path }: { path: string } = $props();

	const group = $derived(groups.find((g) => g.tabs.some((tab) => tab.href === path)));
	const count = (tab: Tab) => (tab.badge ? counts[tab.badge] : 0);
	const said = (tab: Tab) =>
		tab.badge === 'accounts'
			? t('nav.badge_accounts', { count: count(tab) })
			: t('nav.badge_review', { count: count(tab) });
</script>

{#if group}
	<nav class="tabs" aria-label={t(group.label)}>
		{#each group.tabs as tab (tab.href)}
			<a
				href={tab.href}
				aria-current={tab.href === path ? 'page' : undefined}
				aria-describedby={count(tab) ? `tab-${tab.badge}` : undefined}
			>
				<span class="name"
					>{t(tab.label)}
					{#if tab.badge}<Badge
							count={count(tab)}
							tone={tab.badge === 'accounts' ? 'warn' : 'bad'}
						/>{/if}</span
				>
				<span class="line" aria-hidden="true"></span>
			</a>
			{#if count(tab)}<span id="tab-{tab.badge}" hidden>{said(tab)}</span>{/if}
		{/each}
	</nav>
{/if}

<style>
	.tabs {
		display: grid;
		grid-auto-flow: column;
		grid-auto-columns: minmax(0, max-content);
		gap: var(--space-xs);
		margin-bottom: var(--space-xl);
		border-bottom: 1px solid var(--border);
	}
	a {
		position: relative;
		display: grid;
		place-items: center;
		min-height: 48px;
		padding: 0 var(--space-lg);
		color: var(--muted);
		font-weight: 600;
		text-decoration: none;
	}
	a[aria-current='page'] {
		color: var(--text);
	}
	@media (hover: hover) {
		a:hover {
			color: var(--text);
		}
	}
	.name {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
	}
	.line {
		position: absolute;
		left: var(--space-md);
		right: var(--space-md);
		bottom: -1px;
		height: 3px;
		border-radius: 3px 3px 0 0;
		background: var(--accent);
		opacity: 0;
		transform: scaleX(0.5);
		transition: opacity var(--duration-medium) var(--ease-standard);
	}
	a[aria-current='page'] .line {
		opacity: 1;
		transform: none;
	}
	@media (prefers-reduced-motion: no-preference) {
		.line {
			transition:
				opacity var(--duration-medium) var(--ease-standard),
				transform var(--duration-medium) var(--ease-standard);
		}
	}
	/* A phone: the two tabs share the width. */
	@media (max-width: 899px) {
		.tabs {
			grid-auto-columns: minmax(0, 1fr);
			margin-bottom: var(--space-lg);
		}
	}
	/* The narrowest phones: a smaller label and padding keep each tab on one line. */
	@media (max-width: 359px) {
		a {
			padding: 0 var(--space-sm);
			font-size: var(--text-sm);
			white-space: nowrap;
		}
	}
</style>
