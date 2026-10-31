<script lang="ts">
	import { tick } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { day, money } from '#lib/format.ts';
	import { plural, t } from '#lib/i18n.svelte.ts';
	import { glide, rise, vanish } from '#lib/motion.ts';
	import Money from '#lib/ui/Money.svelte';
	import { notify } from '#lib/ui/notices.svelte.ts';
	import { categoryName, payeeName, type Names } from '#lib/rules/describe.ts';

	let { names, onapplied }: { names: Names; onapplied: () => void } = $props();

	let preview = $state<Schemas['RulePreview'] | null>(null);
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
		if (busy) return;
		busy = true;
		const { data, error: failure, response } = await api.POST('/api/v1/rules/preview');
		busy = false;
		if (!data) {
			preview = null;
			notify(problem(failure, response?.status, t('changes.check_failed')), 'bad');
			return;
		}
		preview = data;
	}

	async function apply() {
		if (!preview) return;
		busy = true;
		const {
			data,
			error: failure,
			response
		} = await api.POST('/api/v1/rules/apply', {
			params: { header: { 'If-Match': preview.tag } }
		});
		busy = false;
		if (!data) {
			notify(
				response?.status === 412
					? t('changes.stale')
					: problem(failure, response?.status, t('changes.apply_failed')),
				'bad'
			);
			return;
		}
		notify(plural(data.applied, 'changes.applied_one', 'changes.applied_other'));
		onapplied();
		await check();
		// The apply button is gone with the changes: the focus goes to what the list says now.
		await tick();
		document.getElementById(preview?.changes.length ? 'changes-some' : 'changes-none')?.focus();
	}
</script>

<section class="card" aria-labelledby="changes-title">
	<h2 id="changes-title">{t('changes.title')}</h2>
	<p class="hint">{t('changes.hint')}</p>
	<div class="actions">
		<!-- aria-disabled, not disabled: a disabled button drops the focus to the page while it works. -->
		<button
			class="secondary"
			onclick={() => {
				check();
			}}
			aria-disabled={busy}>{t('changes.check')}</button
		>
	</div>
	{#if preview}
		<!-- `global`: these also enter when the first check opens the block around them. -->
		{#if preview.changes.length === 0}
			<p id="changes-none" tabindex="-1" in:rise|global out:vanish>{t('changes.nothing')}</p>
		{:else}
			<p id="changes-some" tabindex="-1" in:rise|global out:vanish>
				{plural(preview.changes.length, 'changes.would_one', 'changes.would_other')}
			</p>
			<ul class="changes" in:rise|global out:vanish>
				{#each preview.changes as change (change.transaction_id)}
					<li out:vanish animate:glide>
						<div class="txn-line">
							<span>{day(change.date)}</span>
							<span class="description">{change.description}</span>
							<Money minor={change.amount_minor} currency={change.currency} />
						</div>
						<div class="hint">{what(change).join(' · ')}</div>
					</li>
				{/each}
			</ul>
			<div class="actions" in:rise|global out:vanish>
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
		gap: var(--space-sm);
	}
	.changes li {
		border-top: 1px solid var(--border);
		padding-top: var(--space-sm);
	}
</style>
