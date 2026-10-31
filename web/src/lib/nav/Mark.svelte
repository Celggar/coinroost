<!-- The login's mark: a green rounded square with a roof and a dot. -->
<svg class="mark" aria-hidden="true" width="32" height="32" viewBox="0 0 32 32">
	<rect width="32" height="32" rx="9" class="tile" />
	<path d="M9 17l7-6 7 6" class="roof" />
	<circle cx="16" cy="20.5" r="3" class="dot" />
</svg>

<style>
	.mark {
		flex: none;
	}
	.tile {
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
</style>
