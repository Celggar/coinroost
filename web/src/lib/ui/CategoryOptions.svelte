<script lang="ts" module>
	import type { Schemas } from '#lib/api/client.ts';
	import type { Key } from '#lib/i18n.svelte.ts';

	/** What a category holds, in words, spending first: the form's choices and the selects' groups, in this order. */
	export const holds = {
		expense: 'categoryform.expense',
		income: 'categoryform.income',
		transfer: 'categoryform.transfer',
		opening_balance: 'categoryform.opening_balance'
	} satisfies Record<Schemas['CategoryKind'], Key>;
</script>

<script lang="ts">
	// A select's categories by what they hold: in one list «Salario» stood among the spending. A household with one kind
	// gets no heading, which would only repeat it.
	import { t } from '#lib/i18n.svelte.ts';

	let { categories }: { categories: Schemas['Category'][] } = $props();

	const kinds = Object.keys(holds) as Schemas['CategoryKind'][];
	const groups = $derived(
		kinds
			.map((kind) => ({ kind, items: categories.filter((category) => category.kind === kind) }))
			.filter(({ items }) => items.length > 0)
	);
</script>

{#snippet options(items: Schemas['Category'][])}
	{#each items as category (category.id)}
		<option value={category.id}>{category.name}</option>
	{/each}
{/snippet}

{#if groups.length > 1}
	{#each groups as { kind, items } (kind)}
		<optgroup label={t(holds[kind])}>{@render options(items)}</optgroup>
	{/each}
{:else}
	{@render options(categories)}
{/if}
