<script lang="ts">
	// «Instalar la app» on «Más»: the browser's own install dialog where it has one, the way through the Share menu on an
	// iPhone or iPad, and nothing once the app is installed or where neither is there.
	import { t } from '#lib/i18n.svelte.ts';
	import { fromShareMenu, install, offerInstall } from '#lib/install.svelte.ts';
	import { grow } from '#lib/motion.ts';

	const shareMenu = fromShareMenu();
</script>

{#if !install.installed && (install.prompt || shareMenu)}
	<!-- It can come after the page, when the browser hands its prompt over late: it opens its room instead of pushing the
	     log-out button down in a frame. -->
	<section class="card install" aria-labelledby="install-title" transition:grow>
		<h2 id="install-title">{t('install.title')}</h2>
		<p class="hint">{install.prompt ? t('install.hint') : t('install.share')}</p>
		{#if install.prompt}
			<button class="secondary" onclick={offerInstall}>{t('install.button')}</button>
		{/if}
	</section>
{/if}

<style>
	.install {
		display: grid;
		gap: var(--space-sm);
		justify-items: start;
	}
	.install h2,
	.install p {
		margin: 0;
	}
</style>
