<script lang="ts">
	import '../app.css';
	import { goto, onNavigate } from '$app/navigation';
	import { page } from '$app/state';
	import LanguageSwitch from '#lib/LanguageSwitch.svelte';
	import Notices from '#lib/ui/Notices.svelte';
	import { i18n, t, type Key } from '#lib/i18n.svelte.ts';
	import { logOut, refresh, session } from '#lib/session.svelte.ts';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
	let barHeight = $state(0);

	const links: { href: string; label: Key }[] = [
		{ href: '/', label: 'nav.summary' },
		{ href: '/import', label: 'nav.import' },
		{ href: '/uncategorized', label: 'nav.uncategorized' },
		{ href: '/rules', label: 'nav.rules' },
		{ href: '/transfers', label: 'nav.transfers' },
		{ href: '/coverage', label: 'nav.coverage' },
		{ href: '/accounts', label: 'nav.accounts' },
		{ href: '/security', label: 'nav.security' }
	];
	// Pages a person opens without a session: the login and a setup link.
	const onPublic = $derived(page.url.pathname === '/login' || page.url.pathname === '/setup');
	// After a recovery code the server lets the session do nothing but set up a new authenticator, so that page is
	// the only one shown, without the app's bar.
	const onAuthenticator = $derived(page.url.pathname === '/authenticator');
	const limited = $derived(session.user?.reenroll_required ?? false);

	$effect(() => {
		document.documentElement.lang = i18n.locale;
	});

	$effect(() => {
		if (!session.checked) {
			refresh();
		} else if (!session.user && !onPublic) {
			const next = page.url.pathname + page.url.search;
			goto(`/login?next=${encodeURIComponent(next)}`, { replaceState: true });
		} else if (session.user && limited !== onAuthenticator && !onPublic) {
			goto(limited ? '/authenticator' : '/', { replaceState: true });
		}
	});

	// Pages cross-fade into each other where the browser has view transitions (SvelteKit's recipe), so nothing
	// appears or disappears in one frame.
	onNavigate((navigation) => {
		if (!document.startViewTransition) return;
		return new Promise((resolve) => {
			document.startViewTransition(async () => {
				resolve();
				await navigation.complete;
			});
		});
	});

	async function leave() {
		await logOut();
		await goto('/login');
	}
</script>

<svelte:head>
	<title>{t('app.name')}</title>
</svelte:head>

{#if session.user && !onPublic && !limited && !onAuthenticator}
	<header class="top" bind:clientHeight={barHeight}>
		<a class="brand" href="/">
			<svg aria-hidden="true" width="28" height="28" viewBox="0 0 32 32">
				<rect width="32" height="32" rx="9" class="mark" />
				<path d="M9 17l7-6 7 6" class="roof" />
				<circle cx="16" cy="20.5" r="3" class="dot" />
			</svg>
			{t('app.name')}
		</a>
		<nav aria-label={t('nav.main')}>
			{#each links as link (link.href)}
				<a href={link.href} aria-current={page.url.pathname === link.href ? 'page' : undefined}>
					{t(link.label)}
				</a>
			{/each}
		</nav>
		<LanguageSwitch />
		<button class="secondary" onclick={leave}>{t('nav.log_out')}</button>
	</header>
{/if}

<!-- The sign-in pages keep their own alert next to the button; the app's pages share one notice. -->
{#if session.user && !onPublic && !limited && !onAuthenticator}
	<Notices offset={barHeight} />
{/if}

{#if onPublic || (session.user && limited === onAuthenticator)}
	<main class:public={onPublic || onAuthenticator}>{@render children()}</main>
{:else}
	<main><p class="hint">{t('app.loading')}</p></main>
{/if}

<style>
	/* The app's bar takes the login's identity: its mark, and a frosted surface that stays on top. */
	.top {
		position: sticky;
		top: 0;
		z-index: 5;
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-sm) var(--space-lg);
		padding: var(--space-sm) var(--space-lg);
		border-bottom: 1px solid var(--card-edge);
		background: color-mix(in oklab, var(--surface) 82%, transparent);
		-webkit-backdrop-filter: blur(16px) saturate(160%);
		backdrop-filter: blur(16px) saturate(160%);
	}
	@media (prefers-reduced-transparency: reduce), (prefers-contrast: more), (forced-colors: active) {
		.top {
			background: var(--surface);
			-webkit-backdrop-filter: none;
			backdrop-filter: none;
		}
	}
	.brand {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
		min-height: 44px;
		font-weight: 700;
		text-decoration: none;
		color: var(--text);
	}
	.mark {
		fill: var(--accent);
	}
	.roof {
		fill: none;
		stroke: var(--accent-text);
		stroke-width: 2.5;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	.dot {
		fill: var(--accent-text);
	}
	nav {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-xs);
		flex: 1;
	}
	/* On a phone: the name and Log out on the first row, the sections on their own row below. */
	@media (max-width: 560px) {
		.brand {
			flex: 1;
		}
		nav {
			order: 3;
			flex-basis: 100%;
		}
	}
	nav a {
		display: inline-flex;
		align-items: center;
		min-height: 44px;
		padding: 0 var(--space-md);
		border-radius: var(--radius-button);
		text-decoration: none;
	}
	nav a[aria-current='page'] {
		background: color-mix(in oklab, var(--accent) 12%, transparent);
		color: var(--text);
		font-weight: 600;
	}
</style>
