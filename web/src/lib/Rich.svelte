<script lang="ts">
	// A catalog text with **bold** parts, shown without {@html}: the parts between ** pairs are strong.
	let { text }: { text: string } = $props();

	const parts = $derived(text.split('**'));
</script>

{#each parts as part, i (i)}{#if i % 2}<strong>{part}</strong>{:else}{part}{/if}{/each}
