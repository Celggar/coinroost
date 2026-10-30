<script lang="ts">
	// The lines behind the login card. They draw themselves in once (the dash only lives while a line draws: a
	// scaled SVG with pathLength draws dashes wrong in Chromium); with reduced motion they only fade in. Their color
	// comes from the shell's --line and --line-opacity.
	//
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
	@media (max-width: 899px) {
		.wide {
			display: none;
		}
		.narrow {
			display: block;
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
