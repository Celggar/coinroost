<script lang="ts">
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { day, money } from '#lib/format.ts';
	import { plural, t } from '#lib/i18n.svelte.ts';
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
		if (fromCategory !== toCategory)
			parts.push(t('changes.category', { from: fromCategory, to: toCategory }));
		const [fromPayee, toPayee] = [payeeName(change.before, names), payeeName(change.after, names)];
		if (fromPayee !== toPayee) parts.push(t('changes.payee', { from: fromPayee, to: toPayee }));
		if (parts.length === 0) parts.push(t('changes.only_rule'));
		return parts;
	}

	async function check() {
		busy = true;
		error = '';
		const { data, error: failure, response } = await api.POST('/api/v1/rules/preview');
		busy = false;
		if (!data) {
			preview = null;
			error = problem(failure, response?.status, t('changes.check_failed'));
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
					? t('changes.stale')
					: problem(failure, response?.status, t('changes.apply_failed'));
			return;
		}
		done = plural(data.applied, 'changes.applied_one', 'changes.applied_other');
		onapplied();
		await check();
	}
</script>

<section class="card" aria-labelledby="changes-title">
	<h2 id="changes-title">{t('changes.title')}</h2>
	<p class="hint">{t('changes.hint')}</p>
	<div>
		<button
			class="secondary"
			onclick={() => {
				done = '';
				check();
			}}
			disabled={busy}>{t('changes.check')}</button
		>
	</div>
	{#if error}<p class="alert" role="alert">{error}</p>{/if}
	{#if done}<p class="notice" role="status">{done}</p>{/if}
	{#if preview}
		{#if preview.changes.length === 0}
			<p>{t('changes.nothing')}</p>
		{:else}
			<p>{plural(preview.changes.length, 'changes.would_one', 'changes.would_other')}</p>
			<ul class="changes">
				{#each preview.changes as change (change.transaction_id)}
					<li>
						<div class="what">
							<span>{day(change.date)}</span>
							<span class="description">{change.description}</span>
							<span class="amount">{money(change.amount_minor, change.currency)}</span>
						</div>
						<div class="hint">{what(change).join(' · ')}</div>
					</li>
				{/each}
			</ul>
			<div>
				<button onclick={apply} disabled={busy}>
					{plural(preview.changes.length, 'changes.apply_one', 'changes.apply_other')}
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
