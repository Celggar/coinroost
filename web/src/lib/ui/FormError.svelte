<script lang="ts">
	// A form's own error, when it belongs to no field: next to the button that sent it, rising in and lifting out.
	// The region stays on the page so screen readers announce what appears in it.
	import { lift, rise } from '#lib/motion.ts';

	let { text }: { text: string } = $props();
</script>

<div role="alert" class="slot">
	{#if text}<p class="alert" in:rise out:lift>{text}</p>{/if}
</div>

<style>
	.slot {
		display: grid;
	}
	.slot:empty {
		display: none;
	}
</style>
