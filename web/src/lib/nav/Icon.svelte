<script lang="ts">
	// Line icons for the navigation, drawn for this app on a 24 px grid (2 px strokes, round ends), so their weight
	// matches the text beside them. Always decorative: the label next to each one names it.
	import type { IconName } from './destinations.ts';

	let { name, size = 24 }: { name: IconName; size?: number } = $props();

	const paths: Record<IconName, string[]> = {
		home: ['M3 11l9-7 9 7', 'M5.5 9.5V20h5v-6h3v6h5V9.5'],
		import: ['M12 15V4', 'M7.5 8.5L12 4l4.5 4.5', 'M4 14v4a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-4'],
		review: [
			'M4 13l2.2-7.4A1 1 0 0 1 7.2 5h9.6a1 1 0 0 1 1 .6L20 13v6a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1z',
			'M4 13h4.5l1.5 2.5h4l1.5-2.5H20'
		],
		bank: ['M3 9.5L12 4l9 5.5', 'M5.5 10v7.5M10 10v7.5M14 10v7.5M18.5 10v7.5', 'M3.5 20h17'],
		rules: ['M4 6h16', 'M7 12h10', 'M10 18h4'],
		more: ['M5 12h.01M12 12h.01M19 12h.01'],
		shield: ['M12 3.5l7 2.8v5.2c0 4.3-2.9 7.6-7 9-4.1-1.4-7-4.7-7-9V6.3z', 'M9 12l2 2 4-4'],
		logout: ['M14 4h4a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-4', 'M10 16l-4-4 4-4', 'M6 12h10'],
		chev: ['M9 6l6 6-6 6'],
		check: ['M5 12.5l4.5 4.5L19 7.5'],
		unknown: [
			'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z',
			'M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6v.6',
			'M12 17h.01'
		]
	};
</script>

<svg
	class="icon"
	class:dots={name === 'more'}
	aria-hidden="true"
	width={size}
	height={size}
	viewBox="0 0 24 24"
>
	{#each paths[name] as d (d)}<path {d} />{/each}
</svg>

<style>
	.icon {
		flex: none;
		fill: none;
		stroke: currentColor;
		stroke-width: 2;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	.dots {
		stroke-width: 3.2;
	}
</style>
