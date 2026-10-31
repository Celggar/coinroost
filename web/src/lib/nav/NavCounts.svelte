<script lang="ts">
	// What the badges say, in words, for screen readers: each destination with a count is described by one of these
	// (aria-describedby), so its name stays «Revisar» and the count is heard after it.
	import { t } from '#lib/i18n.svelte.ts';
	import { counts } from './counts.svelte.ts';

	let { id }: { id: string } = $props();
</script>

<span id="{id}-review" hidden
	>{counts.review ? t('nav.badge_review', { count: counts.review }) : ''}</span
>
<span id="{id}-accounts" hidden
	>{counts.accounts ? t('nav.badge_accounts', { count: counts.accounts }) : ''}</span
>
