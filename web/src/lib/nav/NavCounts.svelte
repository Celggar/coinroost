<script lang="ts">
	// What the badges say, in words, for screen readers: each destination with a count is described by one of these
	// (aria-describedby), so its name stays «Revisar» and the count is heard after it.
	import { plural } from '#lib/i18n.svelte.ts';
	import { counts } from './counts.svelte.ts';

	let { id }: { id: string } = $props();
</script>

<span id="{id}-review" hidden
	>{counts.review
		? plural(counts.review, 'nav.badge_review_one', 'nav.badge_review_other')
		: ''}</span
>
<span id="{id}-accounts" hidden
	>{counts.accounts
		? plural(counts.accounts, 'nav.badge_accounts_one', 'nav.badge_accounts_other')
		: ''}</span
>
