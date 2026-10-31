<script lang="ts">
	// «From → to» between two accounts: a drawn arrow, centred on the text and as tall as it (a font's «→» often sits
	// small and low), read aloud as a word.
	import { t } from '#lib/i18n.svelte.ts';
</script>

<svg class="arrow" viewBox="0 0 24 24" width="1em" height="1em" aria-hidden="true">
	<path d="M4 12h15M13 6l6 6-6 6" />
</svg><span class="visually-hidden">{` ${t('transfer.to')} `}</span>

<style>
	.arrow {
		display: inline-block;
		vertical-align: -0.125em;
		margin-inline: 0.25em;
		fill: none;
		stroke: currentColor;
		stroke-width: 2;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
</style>
