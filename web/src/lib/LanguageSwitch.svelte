<script lang="ts">
	import { i18n, setLocale, t, type Locale } from '#lib/i18n.svelte.ts';

	// Each language is named in itself, so a person who cannot read the current one still finds theirs.
	const languages: { code: Locale; name: string }[] = [
		{ code: 'es', name: 'Español' },
		{ code: 'en', name: 'English' }
	];
</script>

<label class="language">
	<span class="visually-hidden">{t('language.label')}</span>
	<select value={i18n.locale} onchange={(event) => setLocale(event.currentTarget.value as Locale)}>
		{#each languages as language (language.code)}
			<option value={language.code} lang={language.code}>{language.name}</option>
		{/each}
	</select>
</label>

<style>
	.language select {
		width: auto;
	}
</style>
