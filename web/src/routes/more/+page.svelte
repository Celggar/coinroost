<script lang="ts">
	// «Más» on a phone (P1): what is used rarely — Reglas, Seguridad, the language and logging out — one tap from
	// the bar, never an overflow of hidden destinations.
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import LanguageSwitch from '#lib/LanguageSwitch.svelte';
	import { t } from '#lib/i18n.svelte.ts';
	import Icon from '#lib/nav/Icon.svelte';
	import { logOut, session } from '#lib/session.svelte.ts';

	// On a wide screen the rail holds all of this, so «Más» is no page there: a wide window, or a phone window made
	// wide, goes on to the summary instead of showing a page the rail marks nowhere (N5).
	onMount(() => {
		const wide = matchMedia('(min-width: 900px)');
		const away = () => {
			if (wide.matches) goto('/', { replaceState: true });
		};
		away();
		wide.addEventListener('change', away);
		return () => wide.removeEventListener('change', away);
	});

	async function leave() {
		await logOut();
		await goto('/login');
	}
</script>

<svelte:head>
	<title>{t('app.title', { page: t('more.title') })}</title>
</svelte:head>

<h1>{t('more.title')}</h1>

<div class="stack">
	<nav class="card rows" aria-label={t('more.title')}>
		<a href="/rules"
			><Icon name="rules" /><span>{t('nav.rules')}</span><Icon name="chev" size={20} /></a
		>
		<a href="/security"
			><Icon name="shield" /><span>{t('nav.security')}</span><Icon name="chev" size={20} /></a
		>
	</nav>

	<section class="card" aria-labelledby="language-title">
		<h2 id="language-title">{t('more.language')}</h2>
		<LanguageSwitch wide />
	</section>

	<button class="secondary leave" onclick={leave}><Icon name="logout" />{t('nav.log_out')}</button>
	{#if session.user}
		<p class="hint who">{t('more.signed_in', { email: session.user.email })}</p>
	{/if}
</div>

<style>
	/* As wide as the cards on Security: a wide screen does not stretch the switch and the button across it. */
	.stack {
		max-width: 560px;
	}
	.rows {
		padding: var(--space-xs) 0;
		gap: 0;
	}
	.rows a {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		min-height: 56px;
		padding: 0 var(--space-xl);
		color: var(--text);
		text-decoration: none;
	}
	.rows a + a {
		border-top: 1px solid var(--border);
	}
	.rows span {
		flex: 1;
	}
	.rows a :global(svg:last-child) {
		color: var(--muted);
	}
	.leave {
		width: 100%;
	}
	.who {
		text-align: center;
		overflow-wrap: anywhere;
	}
</style>
