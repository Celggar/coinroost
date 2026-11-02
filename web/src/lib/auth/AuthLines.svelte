<script lang="ts">
	// The lines behind the login card. They draw themselves in once (the dash only lives while a line draws: a
	// scaled SVG with pathLength draws dashes wrong in Chromium); with reduced motion they only fade in. Their color
	// comes from the shell's --line and --line-opacity.
	//
	// Wide screens get long waves across the page; phones get two short bands, top and bottom. The top band stays
	// above the brand row: lower, it crossed «Coinroost» and the headline at mid-height and read as a strike-through
	// (pass 11).
	const wide = [
		'M-40 620 C 220 520, 380 700, 640 560 S 1040 380, 1320 470',
		'M-40 660 C 240 560, 400 740, 660 600 S 1060 420, 1320 510',
		'M-40 700 C 260 600, 420 780, 680 640 S 1080 460, 1320 550',
		'M-40 160 C 200 260, 420 60, 700 180 S 1100 300, 1320 200',
		'M-40 200 C 220 300, 440 100, 720 220 S 1120 340, 1320 240'
	];
	const narrow = [
		'M-20 14 C 70 40, 150 -6, 250 12 S 360 42, 410 22',
		'M-20 32 C 80 54, 160 12, 260 30 S 370 56, 410 40',
		'M-20 760 C 60 700, 150 800, 240 740 S 350 680, 410 720',
		'M-20 792 C 70 732, 160 832, 250 772 S 360 712, 410 752'
	];
</script>

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

<style>
	/* Fixed to the window, not to the page: a card taller than the window makes the page taller, and lines scaled to
	   that height were cut to a loose stroke (pass 1). Behind a scrolling card they stay put, like a backdrop. */
	.lines {
		position: fixed;
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
		animation: fade-in var(--duration-long) var(--ease-enter) backwards;
	}
	.narrow {
		display: none;
	}
	@media (prefers-reduced-motion: no-preference) {
		.lines path {
			animation: draw 560ms var(--ease-enter) backwards;
		}
	}
	/* On a phone the brand scrolls with the page, so its band does too: fixed, a page raised 23 px to show an alert put
	   the band back across «ES» and the logo (pass 12). One screen tall, at the page's top, whatever the card's length. */
	@media (max-width: 899px) {
		.wide {
			display: none;
		}
		.narrow {
			display: block;
			position: absolute;
			bottom: auto;
			height: 100lvh;
		}
	}
	/* Decoration only: with the system's colors forced they kept the brand's green, outside the palette the person
	   chose (pass 15), so they go. Two classes, to beat the phone's `display: block` above. */
	@media (forced-colors: active) {
		.lines.wide,
		.lines.narrow {
			display: none;
		}
	}
	@keyframes fade-in {
		from {
			opacity: 0;
		}
	}
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
