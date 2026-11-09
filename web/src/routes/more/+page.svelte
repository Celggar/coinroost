<script lang="ts">
	// «Más» on a phone (P1): what is used rarely — Reglas, Seguridad, Comentarios, the language and logging out — one tap from
	// the bar, never an overflow of hidden destinations.
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import LanguageSwitch from '#lib/LanguageSwitch.svelte';
	import { t } from '#lib/i18n.svelte.ts';
	import Icon from '#lib/nav/Icon.svelte';
	import InstallApp from '#lib/nav/InstallApp.svelte';
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
		<span class="divider" aria-hidden="true"></span>
		<a href="/security"
			><Icon name="shield" /><span>{t('nav.security')}</span><Icon name="chev" size={20} /></a
		>
		<span class="divider" aria-hidden="true"></span>
		<a href="/feedback"
			><Icon name="feedback" /><span>{t('feedback.title')}</span><Icon name="chev" size={20} /></a
		>
	</nav>

	<section class="card language-card" aria-labelledby="language-title">
		<h2 id="language-title">{t('more.language')}</h2>
		<LanguageSwitch wide />
	</section>

	<InstallApp />

	<button class="secondary leave" onclick={leave}><Icon name="logout" />{t('nav.log_out')}</button>
	{#if session.user}
		{@const [before, after] = t('more.signed_in', { email: '\u0001' }).split('\u0001')}
		{@const at = session.user.email.lastIndexOf('@') + 1}
		<p class="hint who">
			{before}<span class="address"
				><span>{session.user.email.slice(0, at)}</span><wbr /><span
					>{session.user.email.slice(at)}</span
				></span
			>{after}
		</p>
	{/if}
</div>

<style>
	/* As wide as the cards on Security: a wide screen does not stretch the switch and the button across it. */
	.stack {
		max-width: 560px;
	}
	/* On the phone layout the column sits on the bottom bar's axis, not left of it (pass 7, at 899 px). */
	@media (max-width: 899px) {
		.stack {
			margin-inline: auto;
		}
	}
	/* The rows sit 4 px in from the card, with 4 px, the divider and 4 px between them: the focus ring, 2 px inside a
	   row, keeps 6 px from the card's edges and from the divider alike (it was 6 px from the top and 2 px from the
	   sides and the divider, P2 round 9). */
	.rows {
		padding: var(--space-xs);
		gap: var(--space-xs);
	}
	/* Its own element, not a link's border: under forced colors a link's lines take the link color. It reaches the
	   card's edges. */
	.divider {
		position: relative;
		left: calc(-1 * var(--space-xs));
		width: calc(100% + 2 * var(--space-xs));
		border-top: 1px solid var(--border);
	}
	/* The focus ring is drawn inside the row and rounded, so it neither leaves the card at its corners nor covers the
	   divider; the divider is its own line, which the rounding does not bend. */
	.rows a {
		position: relative;
		border-radius: var(--radius-button);
		display: flex;
		align-items: center;
		gap: var(--space-md);
		min-height: 56px;
		/* The icons' strokes start and the chevrons' end on the language card's text edges (16 px in): the icon's
		   stroke starts 4 px into its box and the chevron's ends 8 px before its own; the row is 4 px in already. */
		padding: 0 calc(var(--space-sm) - var(--space-xs)) 0 calc(var(--space-md) - var(--space-xs));
		color: var(--text);
		text-decoration: none;
	}
	.rows a:focus-visible {
		outline-offset: -5px;
	}
	.rows span {
		flex: 1;
	}
	.rows a :global(svg:last-child) {
		color: var(--muted);
	}
	/* Forced colors leave an SVG's author color alone: the chevron takes the row's system link color. */
	@media (forced-colors: active) {
		.rows a :global(svg:last-child) {
			color: inherit;
		}
	}
	/* The language card keeps a phone card's 16 px at every width: its title and switch start where the rows' icons
	   do. A wider card's 24 px put them 9 px in from the icons between 561 and 899 px (P2 round 10). */
	.language-card {
		padding: var(--space-lg);
	}
	.leave {
		width: 100%;
	}
	.who {
		text-align: center;
		overflow-wrap: anywhere;
	}
	/* The address keeps together, on a line of its own when it does not fit beside the words; longer than the line,
	   it breaks after its «@» before any hyphen of its name, as in the account panel (pass 7). */
	.address {
		display: inline-block;
		max-width: 100%;
	}
	/* The domain holds together too: at its own hyphen it left «…@ejemplo-» / «largo.test» (pass 26). */
	.address > span:last-child {
		display: inline-block;
		max-width: 100%;
	}
</style>
