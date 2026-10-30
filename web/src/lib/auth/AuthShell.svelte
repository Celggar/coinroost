<script lang="ts">
	// The frame of the pages a person sees before logging in: lines and two soft glows behind a glass card, the
	// brand beside it (above it on a phone) and the language pills. The page fills the card.
	//
	// Glass is kept nearly opaque (80 %) because no iPhone browser honors prefers-reduced-transparency: the card has
	// to pass contrast on its own. The QR and fields sit on solid surfaces, never on the glass itself. Everything
	// arrives with a one-time CSS animation (Svelte intros depend on how the page mounted); with reduced motion it
	// only fades.
	import type { Snippet } from 'svelte';
	import LanguageSwitch from '#lib/LanguageSwitch.svelte';
	import { t } from '#lib/i18n.svelte.ts';

	let { children }: { children: Snippet } = $props();

	// Wide screens get long waves across the page; phones get two short bands, top and bottom.
	const wide = [
		'M-40 620 C 220 520, 380 700, 640 560 S 1040 380, 1320 470',
		'M-40 660 C 240 560, 400 740, 660 600 S 1060 420, 1320 510',
		'M-40 700 C 260 600, 420 780, 680 640 S 1080 460, 1320 550',
		'M-40 160 C 200 260, 420 60, 700 180 S 1100 300, 1320 200',
		'M-40 200 C 220 300, 440 100, 720 220 S 1120 340, 1320 240'
	];
	const narrow = [
		'M-20 92 C 70 140, 150 40, 250 90 S 360 150, 410 110',
		'M-20 122 C 80 170, 160 70, 260 120 S 370 180, 410 140',
		'M-20 760 C 60 700, 150 800, 240 740 S 350 680, 410 720',
		'M-20 792 C 70 732, 160 832, 250 772 S 360 712, 410 752'
	];
</script>

<div class="shell">
	<svg
		class="lines wide"
		aria-hidden="true"
		viewBox="0 0 1280 800"
		preserveAspectRatio="xMidYMid slice"
	>
		{#each wide as d, i (d)}
			<path {d} pathLength="1" style:animation-delay="{i * 70}ms" />
		{/each}
	</svg>
	<svg
		class="lines narrow"
		aria-hidden="true"
		viewBox="0 0 390 844"
		preserveAspectRatio="xMidYMid slice"
	>
		{#each narrow as d, i (d)}
			<path {d} pathLength="1" style:animation-delay="{i * 70}ms" />
		{/each}
	</svg>

	<div class="language"><LanguageSwitch /></div>

	<div class="layout">
		<div class="brand">
			<p class="logo">
				<svg aria-hidden="true" width="36" height="36" viewBox="0 0 32 32">
					<rect width="32" height="32" rx="9" class="mark" />
					<path d="M9 17l7-6 7 6" class="roof" />
					<circle cx="16" cy="20.5" r="3" class="dot" />
				</svg>
				{t('app.name')}
			</p>
			<p class="tagline">{t('auth.tagline')}</p>
		</div>
		<div class="stage">
			<div class="glow first" aria-hidden="true"></div>
			<div class="glow second" aria-hidden="true"></div>
			<section class="glass">
				{@render children()}
			</section>
		</div>
	</div>
</div>

<style>
	.shell {
		--shell-bg: linear-gradient(135deg, #f0fdf4 0%, #b9f8cf 100%);
		--line: #05df72;
		--line-opacity: 0.75;
		--glow-first: #a4f4cf;
		--glow-second: #00d492;
		--glow-second-opacity: 0.55;
		--glass: rgb(255 255 255 / 0.8);
		--glass-edge: rgb(255 255 255 / 0.6);
		--glass-shine: rgb(255 255 255 / 0.9);
		--glass-shadow: 0 12px 40px rgb(3 46 21 / 0.12);
		--brand-text: #032e15;
		--ring-gap: #ffffff;
		position: relative;
		min-height: 100dvh;
		overflow: hidden;
		isolation: isolate;
		background: var(--shell-bg);
		color: var(--text);
	}
	@media (prefers-color-scheme: dark) {
		.shell {
			--shell-bg: linear-gradient(135deg, #0a0f0c 0%, #0f1a13 100%);
			--line: #008236;
			--line-opacity: 0.9;
			--glow-first: #007a55;
			--glow-second: #004f3b;
			--glow-second-opacity: 0.9;
			--glass: rgb(9 11 12 / 0.8);
			--glass-edge: rgb(255 255 255 / 0.1);
			--glass-shine: rgb(255 255 255 / 0.08);
			--glass-shadow: 0 20px 50px rgb(0 0 0 / 0.45);
			--brand-text: #f9fbfb;
			--ring-gap: #0c100f;
		}
	}

	.lines {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		z-index: -1;
		fill: none;
		stroke: var(--line);
		stroke-width: 2;
		opacity: var(--line-opacity);
	}
	.lines path {
		animation: fade-in var(--duration-long) var(--ease-standard) backwards;
	}
	.narrow {
		display: none;
	}

	.language {
		position: absolute;
		top: 24px;
		right: 32px;
		z-index: 1;
	}

	.layout {
		position: relative;
		box-sizing: border-box;
		min-height: 100dvh;
		max-width: 1280px;
		margin: 0 auto;
		padding: 96px 112px;
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 420px);
		align-items: center;
		gap: 48px;
	}

	.brand {
		color: var(--brand-text);
		max-width: 460px;
		display: grid;
		gap: 20px;
	}
	.brand > * {
		margin: 0;
		animation: fade-in var(--duration-long) var(--ease-enter) backwards;
	}
	.tagline {
		font-size: 2.75rem;
		line-height: 1.1;
		font-weight: 700;
		letter-spacing: -0.02em;
		text-wrap: balance;
		animation-delay: 80ms;
	}
	.logo {
		display: flex;
		align-items: center;
		gap: 12px;
		font-size: 1.375rem;
		font-weight: 650;
	}
	.mark {
		fill: var(--accent);
	}
	.roof {
		fill: none;
		stroke: var(--accent-text);
		stroke-width: 2.5;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	.dot {
		fill: var(--accent-text);
	}

	.stage {
		position: relative;
		min-width: 0;
	}
	.glow {
		position: absolute;
		z-index: -1;
		aspect-ratio: 1;
		border-radius: 50%;
		pointer-events: none;
		animation: fade-in 1100ms var(--ease-standard) backwards;
	}
	.glow.first {
		left: -22%;
		top: -22%;
		width: 105%;
		background: var(--glow-first);
		filter: blur(70px);
		opacity: 0.95;
	}
	.glow.second {
		left: 43%;
		top: 40%;
		width: 90%;
		background: var(--glow-second);
		filter: blur(80px);
		opacity: var(--glow-second-opacity);
		animation-delay: 120ms;
	}

	.glass {
		box-sizing: border-box;
		width: 100%;
		padding: 32px;
		border-radius: 24px;
		background: var(--glass);
		border: 1px solid var(--glass-edge);
		box-shadow:
			inset 0 1px 0 var(--glass-shine),
			var(--glass-shadow);
		/* Literal values: Safari does not resolve variables inside -webkit-backdrop-filter. */
		-webkit-backdrop-filter: blur(16px) saturate(160%);
		backdrop-filter: blur(16px) saturate(160%);
		animation: fade-in var(--duration-long) var(--ease-enter) 160ms backwards;
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: 20px;
	}

	/* What the pages put in the card. */
	.glass :global(h1) {
		margin: 0;
		font-size: 1.5rem;
		font-weight: 650;
		letter-spacing: -0.01em;
	}
	.glass :global(.lead) {
		margin: 6px 0 0;
		font-size: 0.9rem;
		color: var(--muted);
	}
	.glass :global(.step-label) {
		margin: 6px 0 0;
		font-size: 0.8125rem;
		font-weight: 600;
		letter-spacing: 0.02em;
		color: var(--link);
	}
	.glass :global(input:not([type='radio'])) {
		border-radius: 10px;
		font-size: 1rem;
		font-weight: 400;
	}
	.glass :global(button.primary) {
		width: 100%;
		min-height: 48px;
		border-radius: 12px;
		font-weight: 600;
		display: grid;
		place-items: center;
	}
	.glass :global(button.primary > *) {
		grid-area: 1 / 1;
	}
	.glass :global(.actions) {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
	}

	/* Moving entrances only for people who have not asked for less motion; the others get the fades above. */
	@media (prefers-reduced-motion: no-preference) {
		.brand > *,
		.glass {
			animation-name: rise-in;
		}
		.glow {
			animation-name: glow-in;
		}
		.lines path {
			animation: draw 1600ms var(--ease-standard) backwards;
		}
	}

	/* Glass turns solid where people ask for it or the system draws its own colors. */
	@media (prefers-reduced-transparency: reduce), (prefers-contrast: more), (forced-colors: active) {
		.glass {
			background: var(--surface);
			border-color: var(--field-border);
			-webkit-backdrop-filter: none;
			backdrop-filter: none;
		}
	}

	@media (max-width: 899px) {
		.wide {
			display: none;
		}
		.narrow {
			display: block;
		}
		.language {
			top: 56px;
			right: 16px;
		}
		.layout {
			grid-template-columns: minmax(0, 1fr);
			align-content: start;
			align-items: start;
			gap: 16px;
			padding: 56px 16px 24px;
		}
		.brand {
			gap: 16px;
		}
		.logo {
			min-height: 44px;
			font-size: 1.2rem;
		}
		.logo svg {
			width: 30px;
			height: 30px;
		}
		.tagline {
			font-size: 1.625rem;
			line-height: 1.15;
		}
		.glass {
			padding: 24px;
		}
		.glass :global(input:not([type='radio'])) {
			min-height: 48px;
			font-size: 1.0625rem;
		}
		.glass :global(button.primary) {
			min-height: 52px;
			border-radius: 14px;
			font-size: 1.0625rem;
		}
		.glow.first {
			left: -24%;
			top: 8%;
			width: 90%;
		}
		.glow.second {
			left: 40%;
			top: 55%;
			width: 80%;
		}
	}

	@keyframes fade-in {
		from {
			opacity: 0;
		}
	}
	@keyframes rise-in {
		from {
			opacity: 0;
			transform: translateY(18px) scale(0.98);
		}
	}
	@keyframes glow-in {
		from {
			opacity: 0;
			transform: scale(0.8);
		}
	}
	/* The dash only lives while the line draws: a scaled SVG with pathLength draws dashes wrong in Chromium. */
	@keyframes draw {
		from {
			stroke-dasharray: 1;
			stroke-dashoffset: 1;
		}
		to {
			stroke-dasharray: 1;
			stroke-dashoffset: 0;
		}
	}
</style>
