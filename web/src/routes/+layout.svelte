<script lang="ts">
	import '../app.css';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import LanguageSwitch from '#lib/LanguageSwitch.svelte';
	import { i18n, t, type Key } from '#lib/i18n.svelte.ts';
	import { logOut, refresh, session } from '#lib/session.svelte.ts';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	const links: { href: string; label: Key }[] = [
		{ href: '/', label: 'nav.summary' },
		{ href: '/import', label: 'nav.import' },
		{ href: '/uncategorized', label: 'nav.uncategorized' },
		{ href: '/rules', label: 'nav.rules' },
		{ href: '/transfers', label: 'nav.transfers' },
		{ href: '/coverage', label: 'nav.coverage' },
		{ href: '/accounts', label: 'nav.accounts' }
	];
	// Pages a person opens without a session: the login and a setup link.
	const onPublic = $derived(page.url.pathname === '/login' || page.url.pathname === '/setup');

	$effect(() => {
		document.documentElement.lang = i18n.locale;
	});

	$effect(() => {
		if (!session.checked) {
			refresh();
		} else if (!session.user && !onPublic) {
			const next = page.url.pathname + page.url.search;
			goto(`/login?next=${encodeURIComponent(next)}`, { replaceState: true });
		}
	});

	async function leave() {
		await logOut();
		await goto('/login');
	}
</script>

<svelte:head>
	<title>{t('app.name')}</title>
</svelte:head>

{#if session.user && !onPublic}
	<header class="top">
		<a class="brand" href="/">{t('app.name')}</a>
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

{#if onPublic || session.user}
	<main>{@render children()}</main>
{:else}
	<main><p class="hint">{t('app.loading')}</p></main>
{/if}

<style>
	.top {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px 16px;
		padding: 8px 16px;
		border-bottom: 1px solid var(--border);
		background: var(--surface);
	}
	.brand {
		font-weight: 700;
		text-decoration: none;
		color: var(--text);
	}
	nav {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
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
		padding: 10px 10px;
		border-radius: var(--radius);
		text-decoration: none;
	}
	nav a[aria-current='page'] {
		background: var(--bg);
		color: var(--text);
		font-weight: 600;
	}
</style>
