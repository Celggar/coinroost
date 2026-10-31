<script lang="ts">
	import { tick } from 'svelte';
	import { refreshCounts } from '#lib/nav/counts.svelte.ts';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { money, wholeDay } from '#lib/format.ts';
	import { plural, t } from '#lib/i18n.svelte.ts';
	import { glide, rise, vanish } from '#lib/motion.ts';
	import Arrow from '#lib/ui/Arrow.svelte';
	import { focusOn } from '#lib/ui/focus.ts';
	import { notify } from '#lib/ui/notices.svelte.ts';
	import { accountName, passLabel, type AccountNames } from '#lib/transfers/legs.ts';

	let {
		preview,
		names,
		onchanged
	}: { preview: Schemas['TransferPreview']; names: AccountNames; onchanged: () => Promise<void> } =
		$props();

	let busy = $state(false);

	async function apply() {
		busy = true;
		const {
			data,
			error: failure,
			response
		} = await api.POST('/api/v1/transfers/apply', {
			params: { header: { 'If-Match': preview.tag } }
		});
		busy = false;
		if (!data) {
			notify(
				response?.status === 412
					? t('waiting.stale')
					: problem(failure, response?.status, t('waiting.failed')),
				'bad'
			);
		} else {
			notify(plural(data.paired, 'waiting.paired_one', 'waiting.paired_other'));
			refreshCounts();
		}
		// The pairs and this button leave: the focus goes to the list the pairs join, not to the page's start, once
		// the page has reloaded and the lists have their new size.
		await onchanged();
		await tick();
		if (data) focusOn('#made-title');
	}
</script>

<section class="card" aria-labelledby="waiting-title">
	<h2 id="waiting-title">{t('waiting.title')}</h2>
	<div class="in-place">
		{#if preview.pairs.length === 0}
			<p in:rise out:vanish>{t('waiting.nothing')}</p>
		{:else}
			<div class="stack" in:rise out:vanish>
				<p class="hint">{t('waiting.hint')}</p>
				<ul class="pairs" aria-label={t('waiting.title')}>
					{#each preview.pairs as pair (pair.outflow.transaction_id)}
						<li in:rise out:vanish animate:glide>
							<div class="line">
								<span>
									{accountName(pair.outflow.account_id, names)} ({wholeDay(
										pair.outflow.date
									)})<Arrow />{accountName(pair.inflow.account_id, names)} ({wholeDay(
										pair.inflow.date
									)})
								</span>
								<span class="money">{money(-pair.outflow.amount_minor, pair.outflow.currency)}</span
								>
							</div>
							<div class="hint">
								{t('waiting.found', {
									description: pair.outflow.description,
									why: passLabel(pair.pass)
								})}
							</div>
							{#if pair.drops_manual}
								<span class="chip warn">{t('waiting.drops_manual')}</span>
							{/if}
						</li>
					{/each}
				</ul>
				<div class="actions">
					<button onclick={apply} disabled={busy}
						>{plural(preview.pairs.length, 'waiting.pair_one', 'waiting.pair_other')}</button
					>
				</div>
			</div>
		{/if}
	</div>
</section>

<style>
	.pairs {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: var(--space-sm);
	}
	.pairs li {
		border-top: 1px solid var(--border);
		padding-top: var(--space-sm);
		display: grid;
		gap: var(--space-xs);
		justify-items: start;
	}
	/* The amount ends the first line, as in every list of transactions; the pair's text wraps beside it. */
	.line {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: baseline;
		gap: var(--space-xs) var(--space-md);
		overflow-wrap: anywhere;
	}
</style>
