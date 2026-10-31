<script lang="ts">
	// The two dates of a period, side by side at every width (FD4). An end before the start is marked on the end
	// (FD5); the page keeps its last answer dimmed meanwhile, since its numbers no longer match the fields.
	import { t } from '#lib/i18n.svelte.ts';
	import Field from './Field.svelte';

	let { from = $bindable(), to = $bindable() }: { from: string; to: string } = $props();

	const inverted = $derived(!!from && !!to && from > to);
</script>

<div class="date-range">
	<Field label={t('period.from')} name="from">
		{#snippet children(wiring)}<input type="date" {...wiring} bind:value={from} />{/snippet}
	</Field>
	<Field label={t('period.to')} name="to" error={inverted ? t('period.inverted') : ''}>
		{#snippet children(wiring)}<input type="date" {...wiring} bind:value={to} />{/snippet}
	</Field>
</div>
