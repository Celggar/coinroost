<script lang="ts">
	// «Instalar la app» on «Más»: the browser's own install dialog where it has one, the way through the Share menu on an
	// iPhone or iPad, and nothing once the app is installed or where neither is there.
	import { t } from '#lib/i18n.svelte.ts';
	import { fromShareMenu, install, offerInstall } from '#lib/install.svelte.ts';
	import { grow } from '#lib/motion.ts';

	const shareMenu = fromShareMenu();
	const shown = $derived(!install.installed && (install.prompt || shareMenu));
	let card: HTMLElement | undefined = $state();

	// The card leaves with its button, once pressed or once the app is installed, and the browser dropped the focus to the
	// page (pass 72). Before the card goes, the focus moves to the title of the card above, else to the page's.
	$effect.pre(() => {
		if (shown || !card?.contains(document.activeElement)) return;
		const title =
			card.previousElementSibling?.querySelector<HTMLElement>('h2') ??
			document.querySelector<HTMLElement>('main h1');
		if (!title) return;
		title.tabIndex = -1;
		title.focus({ preventScroll: true });
	});
</script>

{#if shown}
	<!-- It can come after the page, when the browser hands its prompt over late: it opens its room instead of pushing the
	     log-out button down in a frame. -->
	<section class="card install" aria-labelledby="install-title" bind:this={card} transition:grow>
		<h2 id="install-title">{t('install.title')}</h2>
		<p class="hint">{install.prompt ? t('install.hint') : t('install.share')}</p>
		{#if install.prompt}
			<button class="secondary" onclick={offerInstall}>{t('install.button')}</button>
		{/if}
	</section>
{/if}

<style>
	/* 16 px between its parts, as the language card above and the cards on Security have (pass 72: 8 px read cramped). */
	.install {
		display: grid;
		gap: var(--space-lg);
		justify-items: start;
	}
	.install h2,
	.install p {
		margin: 0;
	}
</style>
