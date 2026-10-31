<script lang="ts">
	import { tick } from 'svelte';
	import { i18n, setLocale, t, type Locale } from '#lib/i18n.svelte.ts';
	import { pop } from '#lib/motion.ts';

	// `wide`: the full names across the width, as a segmented control (the «Más» page), the chosen one filled and
	// checked like the period presets (Material 3); otherwise the short pills. A place whose background is the
	// surface sets `--language-chosen`, so the chosen pill still shows on it.
	let { wide = false }: { wide?: boolean } = $props();

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

<fieldset class="language" class:wide>
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
			{#if wide}
				{#if i18n.locale === language.code}<svg
						class="check"
						viewBox="0 0 24 24"
						aria-hidden="true"
						in:pop><path d="M5 12.5l4.5 4.5L19 7.5" /></svg
					>{/if}<span lang={language.code}>{language.name}</span>
			{:else}
				<span aria-hidden="true">{language.short}</span>
				<span class="visually-hidden" lang={language.code}>{language.name}</span>
			{/if}
		</label>
	{/each}
</fieldset>

<style>
	.language {
		display: inline-flex;
		gap: var(--space-2xs);
		margin: 0;
		padding: 0;
		border: 0;
		min-width: 0;
	}
	.wide {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		width: 100%;
		padding: var(--space-2xs);
		border: 1px solid var(--field-border);
		border-radius: 999px;
	}
	.wide label {
		display: flex;
		justify-content: center;
		align-items: center;
		gap: var(--space-xs);
		min-height: 44px;
		border-radius: 999px;
		font-size: var(--text-md);
	}
	.wide label.chosen {
		background: color-mix(in oklab, var(--accent) 22%, var(--surface));
	}
	.check {
		flex: none;
		width: 18px;
		height: 18px;
		fill: none;
		stroke: currentColor;
		stroke-width: 2.5;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	label {
		position: relative;
		display: inline-grid;
		place-items: center;
		min-width: 44px;
		min-height: 44px;
		padding: 0 var(--space-md);
		border-radius: 10px;
		font-weight: 600;
		font-size: var(--text-sm);
		color: var(--muted);
		cursor: pointer;
		transition:
			background-color var(--duration-short) var(--ease-standard),
			color var(--duration-short) var(--ease-standard);
	}
	label.chosen {
		color: var(--text);
		background: var(--language-chosen, color-mix(in oklab, var(--surface) 70%, transparent));
	}
	/* In the segmented control the half not chosen takes a neutral hover layer (Material 3: 8 % of the text color),
	   not the accent: in the dark theme an accent tint read as a second chosen half. The pills mark their choice with
	   a soft fill only, so there any tint would read as a second choice: they darken the label. */
	@media (hover: hover) {
		label:hover:not(.chosen) {
			color: var(--text);
		}
		.wide label:hover:not(.chosen) {
			background: color-mix(in oklab, var(--text) 8%, transparent);
		}
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
	/* Forced colors drop the chosen pill's background: an outline marks it instead. */
	@media (forced-colors: active) {
		label.chosen {
			outline: 1px solid CanvasText;
			outline-offset: -1px;
		}
	}
</style>
