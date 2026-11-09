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
		logout: ['M10 4H6a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h4', 'M14 16l4-4-4-4', 'M18 12H8'],
		chev: ['M9 6l6 6-6 6'],
		check: ['M5 12.5l4.5 4.5L19 7.5'],
		unknown: [
			'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z',
			'M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6v.6',
			'M12 17h.01'
		],
		// The wireless mark struck through: a question mark read as help, not as no connection (pass 73).
		offline: [
			'M3 9.5a12.8 12.8 0 0 1 18 0',
			'M6 12.8a8.5 8.5 0 0 1 12 0',
			'M9 16a4.2 4.2 0 0 1 6 0',
			'M12 19h.01'
		],
		tag: [
			'M3.5 12.6V4.5a1 1 0 0 1 1-1h8.1l7.9 7.9a1.5 1.5 0 0 1 0 2.1l-6.9 6.9a1.5 1.5 0 0 1-2.1 0z',
			'M8 8h.01'
		],
		clock: ['M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z', 'M12 7.5V12l3 2'],
		transfer: ['M4 8h14', 'M14.5 4.5L18 8l-3.5 3.5', 'M20 16H6', 'M9.5 12.5L6 16l3.5 3.5'],
		cash: [
			'M4 6.5h16a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-9a1 1 0 0 1 1-1z',
			'M12 14.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5z'
		]
	};

	// A strike cuts a gap through what it crosses: drawn straight over the arcs, it read as a knot at 28 px (pass 74).
	// The gap is a mask, so it shows the surface under the icon, glass included.
	const strikes: Partial<Record<IconName, string>> = { offline: 'M4 4l16 16' };
	const id = $props.id();
</script>

<svg
	class="icon"
	class:dots={name === 'more'}
	aria-hidden="true"
	width={size}
	height={size}
	viewBox="0 0 24 24"
>
	{#if strikes[name]}
		<mask id="{id}-cut" maskUnits="userSpaceOnUse" x="0" y="0" width="24" height="24">
			<rect width="24" height="24" fill="white" stroke="none" />
			<path d={strikes[name]} stroke="black" stroke-width="5" />
		</mask>
		<g mask="url(#{id}-cut)"
			>{#each paths[name] as d (d)}<path {d} />{/each}</g
		>
		<path d={strikes[name]} />
	{:else}
		{#each paths[name] as d (d)}<path {d} />{/each}
	{/if}
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
