<script lang="ts">
	import { tick } from 'svelte';
	import { i18n, setLocale, t, type Locale } from '#lib/i18n.svelte.ts';

	// Each language is named in itself, so a person who cannot read the current one still finds theirs; the pill
	// shows its short code.
	const languages: { code: Locale; short: string; name: string }[] = [
		{ code: 'es', short: 'ES', name: 'Español' },
		{ code: 'en', short: 'EN', name: 'English' }
	];

	// The whole page cross-fades into the other language where the browser has view transitions.
	function choose(locale: Locale) {
		if (locale === i18n.locale) return;
		if (!document.startViewTransition) {
			setLocale(locale);
			return;
		}
		document.startViewTransition(async () => {
			setLocale(locale);
			await tick();
		});
	}
</script>

<fieldset class="language">
	<legend class="visually-hidden">{t('language.label')}</legend>
	{#each languages as language (language.code)}
		<label class:chosen={i18n.locale === language.code}>
			<input
				type="radio"
				name="language"
				value={language.code}
				checked={i18n.locale === language.code}
				onchange={() => choose(language.code)}
			/>
			<span aria-hidden="true">{language.short}</span>
			<span class="visually-hidden" lang={language.code}>{language.name}</span>
		</label>
	{/each}
</fieldset>

<style>
	.language {
		display: inline-flex;
		gap: 2px;
		margin: 0;
		padding: 0;
		border: 0;
		min-width: 0;
	}
	label {
		position: relative;
		display: inline-grid;
		place-items: center;
		min-width: 44px;
		min-height: 44px;
		padding: 0 12px;
		border-radius: 10px;
		font-weight: 600;
		font-size: 0.95rem;
		color: var(--muted);
		cursor: pointer;
		transition:
			background-color var(--duration-short) var(--ease-standard),
			color var(--duration-short) var(--ease-standard);
	}
	label.chosen {
		color: var(--text);
		background: color-mix(in oklab, var(--surface) 70%, transparent);
	}
	label:hover:not(.chosen) {
		color: var(--text);
	}
	input {
		position: absolute;
		inset: 0;
		margin: 0;
		opacity: 0;
		cursor: pointer;
	}
	label:has(input:focus-visible) {
		outline: 3px solid var(--focus);
		outline-offset: 2px;
	}
</style>
