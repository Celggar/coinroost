<script lang="ts">
	import '../app.css';
	import { goto, onNavigate } from '$app/navigation';
	import { page } from '$app/state';
	import NavBar from '#lib/nav/NavBar.svelte';
	import NavRail from '#lib/nav/NavRail.svelte';
	import Tabs from '#lib/nav/Tabs.svelte';
	import { cameBack, refreshCounts } from '#lib/nav/counts.svelte.ts';
	import Notices from '#lib/ui/Notices.svelte';
	import { arrive, placeholder } from '#lib/motion.ts';
	import { crossFade } from '#lib/ui/cross-fade.ts';
	import { foldClosed, foldCloses, foldOpens } from '#lib/ui/hold-room.ts';
	import { i18n, t } from '#lib/i18n.svelte.ts';
	import { logOut, refresh, session } from '#lib/session.svelte.ts';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
	// The phone's bars: the top one's height clears what the browser scrolls into view, the bottom one's height
	// clears the end of the page; both stay 0 on a wide screen, where the rail stands beside the page.
	let barHeight = $state(0);
	let navHeight = $state(0);
	// Pages a person opens without a session: the login and a setup link.
	const onPublic = $derived(page.url.pathname === '/login' || page.url.pathname === '/setup');
	// After a recovery code the server lets the session do nothing but set up a new authenticator, so that page is
	// the only one shown, without the app's bar.
	const onAuthenticator = $derived(page.url.pathname === '/authenticator');
	const limited = $derived(session.user?.reenroll_required ?? false);

	$effect(() => {
		document.documentElement.lang = i18n.locale;
	});

	// The bars' heights, for scroll-padding (base.css): a field the browser scrolls to is never under one (WCAG 2.4.11).
	$effect(() => {
		document.documentElement.style.setProperty('--bar-height', `${barHeight}px`);
		document.documentElement.style.setProperty('--nav-height', `${navHeight}px`);
	});

	const inApp = $derived(!!session.user && !onPublic && !limited && !onAuthenticator);

	// The badges follow every page change and the moment the session is known; pages that change a count refresh it
	// themselves right away.
	$effect(() => {
		if (inApp && page.url.pathname) refreshCounts();
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
			crossFade(async () => {
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

<svelte:document
	onvisibilitychange={() => inApp && cameBack()}
	onclick={foldOpens}
	ontransitionrun={foldCloses}
	ontransitionend={foldClosed}
/>

<svelte:head>
	<title>{t('app.name')}</title>
</svelte:head>

{#if inApp}
	<a class="skip" href="#main">{t('nav.skip')}</a>
	<!-- The app fades in once the session is known, after «Cargando…» has faded out: it appeared in one frame (P2
	     round 11). Between pages the view transition cross-fades instead; this block stays. -->
	<div class="shell" in:arrive>
		<div class="wide-only"><NavRail path={page.url.pathname} onleave={leave} /></div>
		<div class="column">
			<div class="narrow-only">
				<NavBar path={page.url.pathname} bind:top={barHeight} bind:bottom={navHeight} />
			</div>
			<!-- The sign-in pages keep their own alert next to the button; the app's pages share one notice. -->
			<Notices offset={barHeight} />
			<main id="main" tabindex="-1">
				<Tabs path={page.url.pathname} />
				{@render children()}
			</main>
		</div>
	</div>
{:else if onPublic || (session.user && limited === onAuthenticator)}
	<main class:public={onPublic || onAuthenticator}>{@render children()}</main>
{:else}
	<!-- Not a <main>: while it fades out the shell's main is already there, and a page has one main. -->
	<div class="boot"><p class="hint loading" out:placeholder>{t('app.loading')}</p></div>
{/if}

<style>
	/* A wide screen: the rail on the left, the page beside it. A phone: the page under its top bar, the destinations in
	   a bar fixed at the bottom. */
	.shell {
		--rail-width: 248px;
		display: grid;
		grid-template-columns: var(--rail-width) minmax(0, 1fr);
		min-height: 100dvh;
	}
	/* The page's column: the top bar, the page (centred, 960 px at most), and on a phone the navigation bar last,
	   held to the bottom of the screen, so the end of a page is never under it. */
	.column {
		display: flex;
		flex-direction: column;
		min-width: 0;
		min-height: 100dvh;
	}
	.column > main {
		order: 2;
		flex: 1;
		align-self: center;
		width: 100%;
		margin: 0;
	}
	.narrow-only {
		display: none;
	}
	/* The rail's column carries its surface and edge the whole height of the page; the rail stays on screen over it. */
	.wide-only {
		background: var(--surface);
		border-right: 1px solid var(--card-edge);
	}
	@media (max-width: 899px) {
		.shell {
			display: block;
		}
		.wide-only {
			display: none;
		}
		.narrow-only {
			display: contents;
		}
		/* The top bar carries the page's title; the page keeps its heading for screen readers and the outline. Only
		   the app's main: the sign-in pages have no top bar, so their title stays in sight. */
		#main :global(h1) {
			position: absolute;
			width: 1px;
			height: 1px;
			overflow: hidden;
			clip-path: inset(50%);
			white-space: nowrap;
		}
	}
	main:focus {
		outline: none;
	}
	/* The boot line sits where a page's first line does (base.css, main). Its room is the line's own: the line leaves
	   the flow as it fades, and a box keeping 32 px of padding held the app under it, then let it jump up (pass 21). */
	.boot {
		max-width: 960px;
		margin: 0 auto;
	}
	.boot > p {
		margin: 0;
		padding: var(--space-lg);
	}
	/* First in the tab order, out of sight until it has the focus (WCAG 2.4.1). */
	.skip {
		position: fixed;
		z-index: 20;
		top: var(--space-sm);
		left: var(--space-sm);
		padding: var(--space-md) var(--space-lg);
		border-radius: var(--radius-button);
		background: var(--surface);
		color: var(--link);
		font-weight: 600;
		box-shadow: var(--card-shadow);
		transform: translateY(-200%);
	}
	.skip:focus {
		transform: none;
	}
</style>
