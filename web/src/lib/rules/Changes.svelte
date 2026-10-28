<script lang="ts">
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { money } from '#lib/format.ts';
	import { categoryName, payeeName, type Names } from '#lib/rules/describe.ts';

	let { names, onapplied }: { names: Names; onapplied: () => void } = $props();

	let preview = $state<Schemas['RulePreview'] | null>(null);
	let error = $state('');
	let done = $state('');
	let busy = $state(false);

	/** Only what changes: the category, the payee, or both. */
	function what(change: Schemas['RuleChange']): string[] {
		const parts: string[] = [];
		const fromCategory = categoryName(change.before.category_id, names);
		const toCategory = categoryName(change.after.category_id, names);
		if (fromCategory !== toCategory) parts.push(`Category: ${fromCategory} → ${toCategory}`);
		const [fromPayee, toPayee] = [payeeName(change.before, names), payeeName(change.after, names)];
		if (fromPayee !== toPayee) parts.push(`Payee: ${fromPayee} → ${toPayee}`);
		if (parts.length === 0) parts.push('Only the rule that set it changes');
		return parts;
	}

	async function check() {
		busy = true;
		error = '';
		const { data, error: failure, response } = await api.POST('/api/v1/rules/preview');
		busy = false;
		if (!data) {
			preview = null;
			error = problem(failure, response?.status, 'The changes could not be checked');
			return;
		}
		preview = data;
	}

	async function apply() {
		if (!preview) return;
		busy = true;
		error = '';
		done = '';
		const {
			data,
			error: failure,
			response
		} = await api.POST('/api/v1/rules/apply', {
			params: { header: { 'If-Match': preview.tag } }
		});
		busy = false;
		if (!data) {
			error =
				response?.status === 412
					? 'The changes are not the ones you checked any more: check again.'
					: problem(failure, response?.status, 'The rules could not be applied');
			return;
		}
		done = `Applied ${data.applied} ${data.applied === 1 ? 'change' : 'changes'}.`;
		onapplied();
		await check();
	}
</script>

<section class="card" aria-labelledby="changes-title">
	<h2 id="changes-title">Apply the rules to existing transactions</h2>
	<p class="hint">
		See first what would change. A category or payee you chose by hand is never changed.
	</p>
	<div>
		<button
			class="secondary"
			onclick={() => {
				done = '';
				check();
			}}
			disabled={busy}>Check changes</button
		>
	</div>
	{#if error}<p class="alert" role="alert">{error}</p>{/if}
	{#if done}<p class="notice" role="status">{done}</p>{/if}
	{#if preview}
		{#if preview.changes.length === 0}
			<p>Nothing would change.</p>
		{:else}
			<p>
				{preview.changes.length}
				{preview.changes.length === 1 ? 'transaction' : 'transactions'} would change:
			</p>
			<ul class="changes">
				{#each preview.changes as change (change.transaction_id)}
					<li>
						<div class="what">
							<span>{change.date}</span>
							<span class="description">{change.description}</span>
							<span class="amount">{money(change.amount_minor, change.currency)}</span>
						</div>
						<div class="hint">{what(change).join(' · ')}</div>
					</li>
				{/each}
			</ul>
			<div>
				<button onclick={apply} disabled={busy}>
					Apply {preview.changes.length}
					{preview.changes.length === 1 ? 'change' : 'changes'}
				</button>
			</div>
		{/if}
	{/if}
</section>

<style>
	.changes {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 8px;
	}
	.changes li {
		border-top: 1px solid var(--border);
		padding-top: 8px;
	}
	.what {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 12px;
	}
	.description {
		flex: 1;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.amount {
		font-variant-numeric: tabular-nums;
	}
</style>
