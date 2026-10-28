<script lang="ts">
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { day, money } from '#lib/format.ts';
	import { plural, t } from '#lib/i18n.svelte.ts';
	import { accountName, passLabel, type AccountNames } from '#lib/transfers/legs.ts';

	let {
		preview,
		names,
		onchanged
	}: { preview: Schemas['TransferPreview']; names: AccountNames; onchanged: () => void } = $props();

	let error = $state('');
	let done = $state('');
	let busy = $state(false);

	async function apply() {
		busy = true;
		error = '';
		done = '';
		const {
			data,
			error: failure,
			response
		} = await api.POST('/api/v1/transfers/apply', {
			params: { header: { 'If-Match': preview.tag } }
		});
		busy = false;
		if (!data) {
			error =
				response?.status === 412
					? t('waiting.stale')
					: problem(failure, response?.status, t('waiting.failed'));
		} else {
			done = plural(data.paired, 'waiting.paired_one', 'waiting.paired_other');
		}
		onchanged();
	}
</script>

<section class="card" aria-labelledby="waiting-title">
	<h2 id="waiting-title">{t('waiting.title')}</h2>
	{#if error}<p class="alert" role="alert">{error}</p>{/if}
	{#if done}<p class="notice" role="status">{done}</p>{/if}
	{#if preview.pairs.length === 0}
		<p>{t('waiting.nothing')}</p>
	{:else}
		<p class="hint">{t('waiting.hint')}</p>
		<ul class="pairs" aria-label={t('waiting.title')}>
			{#each preview.pairs as pair (pair.outflow.transaction_id)}
				<li>
					<div class="line">
						<strong>{money(-pair.outflow.amount_minor, pair.outflow.currency)}</strong>
						<span>
							{accountName(pair.outflow.account_id, names)} ({day(pair.outflow.date)}) → {accountName(
								pair.inflow.account_id,
								names
							)} ({day(pair.inflow.date)})
						</span>
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
		<div>
			<button onclick={apply} disabled={busy}
				>{plural(preview.pairs.length, 'waiting.pair_one', 'waiting.pair_other')}</button
			>
		</div>
	{/if}
</section>

<style>
	.pairs {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 8px;
	}
	.pairs li {
		border-top: 1px solid var(--border);
		padding-top: 8px;
		display: grid;
		gap: 4px;
		justify-items: start;
	}
	.line {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 12px;
		overflow-wrap: anywhere;
	}
	strong {
		font-variant-numeric: tabular-nums;
	}
</style>
