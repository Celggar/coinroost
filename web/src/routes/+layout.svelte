<script lang="ts">
	import '../app.css';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { logOut, refresh, session } from '#lib/session.svelte.ts';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	const links = [
		{ href: '/import', label: 'Import' },
		{ href: '/uncategorized', label: 'Uncategorized' },
		{ href: '/rules', label: 'Rules' },
		{ href: '/coverage', label: 'Coverage' },
		{ href: '/accounts', label: 'Accounts' }
	];
	const onLogin = $derived(page.url.pathname === '/login');

	$effect(() => {
		if (!session.checked) {
			refresh();
		} else if (!session.user && !onLogin) {
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
	<title>Coinroost</title>
</svelte:head>

{#if session.user && !onLogin}
	<header class="top">
		<a class="brand" href="/import">Coinroost</a>
		<nav aria-label="Main">
			{#each links as link (link.href)}
				<a href={link.href} aria-current={page.url.pathname === link.href ? 'page' : undefined}>
					{link.label}
				</a>
			{/each}
		</nav>
		<button class="secondary" onclick={leave}>Log out</button>
	</header>
{/if}

{#if onLogin || session.user}
	<main>{@render children()}</main>
{:else}
	<main><p class="hint">Loading…</p></main>
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
