<script lang="ts">
	// A movement of money (FD7): the sign in the text and the direction in its color, tabular figures, never wrapped.
	import { signed } from '#lib/format.ts';

	let { minor, currency }: { minor: number; currency: string } = $props();
</script>

<span class="money" class:in={minor > 0} class:out={minor < 0}>{signed(minor, currency)}</span>
