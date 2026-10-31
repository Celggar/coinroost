<script lang="ts">
	import { refreshCounts } from '#lib/nav/counts.svelte.ts';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { day, money } from '#lib/format.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import { glide, rise, vanish } from '#lib/motion.ts';
	import Confirm from '#lib/ui/Confirm.svelte';
	import { notify } from '#lib/ui/notices.svelte.ts';
	import { accountName, type AccountNames } from '#lib/transfers/legs.ts';

	let {
		transfers,
		names,
		onchanged
	}: { transfers: Schemas['Transaction'][]; names: AccountNames; onchanged: () => void } = $props();

	let busy = $state(false);
	let title = $state<HTMLHeadingElement>();

	function sides(transfer: Schemas['Transaction']) {
		const out = transfer.legs.find((l) => l.amount_minor < 0);
		const inn = transfer.legs.find((l) => l.amount_minor >= 0);
		return { out, inn };
	}

	async function split(transfer: Schemas['Transaction']) {
		busy = true;
		const {
			data,
			error: failure,
			response
		} = await api.POST('/api/v1/transactions/{transaction_id}/split', {
			params: { path: { transaction_id: transfer.id } }
		});
		busy = false;
		if (!data) {
			notify(problem(failure, response?.status, t('made.failed')), 'bad');
			return;
		}
		notify(t('made.split_done', { description: transfer.description }));
		refreshCounts();
		onchanged();
		// The transfer and its buttons leave this list: the focus goes to its title, not to the page's start.
		title?.focus();
	}
</script>

<section class="card" aria-labelledby="made-title">
	<h2 id="made-title" tabindex="-1" bind:this={title}>{t('made.title')}</h2>
	<div class="swap">
		{#if transfers.length === 0}
			<p in:rise out:vanish>{t('made.none')}</p>
		{:else}
			<ul class="made" aria-label={t('made.title')} in:rise out:vanish>
				{#each transfers as transfer (transfer.id)}
					{@const { out, inn } = sides(transfer)}
					<li in:rise out:vanish animate:glide>
						<div class="line">
							<span>{day(transfer.date)}</span>
							{#if out}<strong class="money">{money(-out.amount_minor, out.currency)}</strong>{/if}
							{#if out && inn}
								<span
									>{accountName(out.account_id, names)} → {accountName(inn.account_id, names)}</span
								>
							{/if}
						</div>
						<div class="hint">{transfer.description}</div>
						<div class="buttons">
							<Confirm
								label={t('made.split')}
								named={t('made.split_named', { description: transfer.description })}
								yes={t('made.split_it')}
								no={t('made.keep_it')}
								{busy}
								onconfirm={() => split(transfer)}
							/>
						</div>
					</li>
				{/each}
			</ul>
		{/if}
	</div>
</section>

<style>
	.made {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: var(--space-sm);
	}
	.made li {
		border-top: 1px solid var(--border);
		padding-top: var(--space-sm);
		display: grid;
		gap: var(--space-xs);
	}
	.line {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-xs) var(--space-md);
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
		gap: var(--space-sm);
	}
</style>
