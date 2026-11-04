<script lang="ts">
	import { tick } from 'svelte';
	import { refreshCounts } from '#lib/nav/counts.svelte.ts';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { day, money } from '#lib/format.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import { glide, rise, swapIn, vanish } from '#lib/motion.ts';
	import Confirm from '#lib/ui/Confirm.svelte';
	import Arrow from '#lib/ui/Arrow.svelte';
	import { showWhole } from '#lib/ui/landing.ts';
	import { notify } from '#lib/ui/notices.svelte.ts';
	import { accountName, type AccountNames } from '#lib/transfers/legs.ts';

	let {
		transfers,
		names,
		onchanged
	}: { transfers: Schemas['Transaction'][]; names: AccountNames; onchanged: () => Promise<void> } =
		$props();

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
		// The transfer and its buttons leave this list: the focus goes to its title, not to the page's start. It
		// waits for the page to reload, since the pair the split brings back grows the list above and moved the title
		// out of view (pass 3).
		await onchanged();
		await tick();
		// Without the browser's own scroll, which jumped and left a line of the list above cut at the window's top
		// (pass 14): one smooth landing once the page settles, on the list itself, or its start when it is taller.
		title?.focus({ preventScroll: true });
		if (title) await showWhole(title, title.closest('section') ?? title);
	}
</script>

<section class="card" aria-labelledby="made-title">
	<h2 id="made-title" tabindex="-1" bind:this={title}>{t('made.title')}</h2>
	<div class="in-place">
		{#if transfers.length === 0}
			<p in:swapIn out:vanish>{t('made.none')}</p>
		{:else}
			<ul class="made txn-rows" aria-label={t('made.title')} in:swapIn out:vanish>
				{#each transfers as transfer (transfer.id)}
					{@const { out, inn } = sides(transfer)}
					<li in:rise out:vanish animate:glide>
						<div class="txn-line">
							<span>{day(transfer.date)}</span>
							<span class="description"
								>{#if out && inn}{accountName(out.account_id, names)}<Arrow />{accountName(
										inn.account_id,
										names
									)}{/if}</span
							>
							{#if out}<span class="money">{money(-out.amount_minor, out.currency)}</span>{/if}
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
		gap: var(--space-xs) var(--space-md);
	}
	.hint {
		overflow-wrap: anywhere;
	}
	/* Its gap to the words is the row's 4 px and this: alone, the button nearly touched them (pass 25). */
	.buttons {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-sm);
		margin-top: var(--space-xs);
	}
</style>
