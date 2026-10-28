<script lang="ts">
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { day, money } from '#lib/format.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import { accountName, type AccountNames } from '#lib/transfers/legs.ts';

	let {
		transfers,
		names,
		onchanged
	}: { transfers: Schemas['Transaction'][]; names: AccountNames; onchanged: () => void } = $props();

	let confirming = $state<string | null>(null);
	let error = $state('');
	let done = $state('');
	let busy = $state(false);

	function sides(transfer: Schemas['Transaction']) {
		const out = transfer.legs.find((l) => l.amount_minor < 0);
		const inn = transfer.legs.find((l) => l.amount_minor >= 0);
		return { out, inn };
	}

	async function split(transfer: Schemas['Transaction']) {
		busy = true;
		error = '';
		done = '';
		const {
			data,
			error: failure,
			response
		} = await api.POST('/api/v1/transactions/{transaction_id}/split', {
			params: { path: { transaction_id: transfer.id } }
		});
		busy = false;
		confirming = null;
		if (!data) {
			error = problem(failure, response?.status, t('made.failed'));
			return;
		}
		done = t('made.split_done', { description: transfer.description });
		onchanged();
	}
</script>

<section class="card" aria-labelledby="made-title">
	<h2 id="made-title">{t('made.title')}</h2>
	{#if error}<p class="alert" role="alert">{error}</p>{/if}
	{#if done}<p class="notice" role="status">{done}</p>{/if}
	{#if transfers.length === 0}
		<p>{t('made.none')}</p>
	{:else}
		<ul class="made" aria-label={t('made.title')}>
			{#each transfers as transfer (transfer.id)}
				{@const { out, inn } = sides(transfer)}
				<li>
					<div class="line">
						<span>{day(transfer.date)}</span>
						{#if out}<strong>{money(-out.amount_minor, out.currency)}</strong>{/if}
						{#if out && inn}
							<span
								>{accountName(out.account_id, names)} → {accountName(inn.account_id, names)}</span
							>
						{/if}
					</div>
					<div class="hint">{transfer.description}</div>
					<div class="buttons">
						{#if confirming === transfer.id}
							<button onclick={() => split(transfer)} disabled={busy}>{t('made.split_it')}</button>
							<button class="secondary" onclick={() => (confirming = null)}
								>{t('made.keep_it')}</button
							>
						{:else}
							<button
								class="secondary"
								onclick={() => (confirming = transfer.id)}
								aria-label={t('made.split_named', { description: transfer.description })}
								>{t('made.split')}</button
							>
						{/if}
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</section>

<style>
	.made {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 8px;
	}
	.made li {
		border-top: 1px solid var(--border);
		padding-top: 8px;
		display: grid;
		gap: 4px;
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
	.hint {
		overflow-wrap: anywhere;
	}
	.buttons {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}
</style>
