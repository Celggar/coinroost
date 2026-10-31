<script lang="ts">
	// An address that leads nowhere, or a page that failed to load, said in the app's words and language with the
	// way back to the summary: SvelteKit's own page was a bare «404 Not Found» in English.
	import { page } from '$app/state';
	import { t } from '#lib/i18n.svelte.ts';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import { errorTitle } from '#lib/nav/destinations.ts';
</script>

<svelte:head>
	<title>{t('app.title', { page: t(errorTitle(page.status)) })}</title>
</svelte:head>

<h1>{t(errorTitle(page.status))}</h1>
<EmptyState
	icon="unknown"
	text={page.status === 404 ? t('error.not_found') : t('error.failed')}
	href="/"
	action={t('error.to_summary')}
/>
