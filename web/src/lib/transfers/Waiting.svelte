<script lang="ts">
	import { tick } from 'svelte';
	import { refreshCounts } from '#lib/nav/counts.svelte.ts';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { money, wholeDay } from '#lib/format.ts';
	import { plural, t } from '#lib/i18n.svelte.ts';
	import { followContent, glide, rise, swapIn, vanish } from '#lib/motion.ts';
	import Arrow from '#lib/ui/Arrow.svelte';
	import { focusOn } from '#lib/ui/focus.ts';
	import { showWhole } from '#lib/ui/form.ts';
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
		const title = data ? focusOn('#made-title') : null;
		// The list the pairs joined, or its start when it is taller than the screen.
		if (title) await showWhole(title, title.closest('section') ?? title);
	}

	const withDate = (leg: { account_id: string; date: string }) =>
		`${accountName(leg.account_id, names)}\u00a0(${wholeDay(leg.date)})`;
</script>

<!-- The card shrinks as the pairs leave instead of at once: at the page's end the page slid back with it, where it
     had jumped 50 px in one frame (pass 14). -->
<section class="card follows" aria-labelledby="waiting-title" use:followContent>
	<div class="stack">
		<h2 id="waiting-title">{t('waiting.title')}</h2>
		<div class="in-place">
			{#if preview.pairs.length === 0}
				<p in:swapIn out:vanish>{t('waiting.nothing')}</p>
			{:else}
				<div class="stack" in:swapIn out:vanish>
					<p class="hint">{t('waiting.hint')}</p>
					<ul class="pairs" aria-label={t('waiting.title')}>
						{#each preview.pairs as pair (pair.outflow.transaction_id)}
							<li in:rise out:vanish animate:glide>
								<div class="line">
									<!-- From on one line, to on the next behind its arrow; each date holds to its account's last word.
								     Run together, a long name left the arrow ending a line and a date alone on the last (pass 8). -->
									<span class="route">
										<span>{withDate(pair.outflow)}</span>
										<span class="to"><Arrow />{withDate(pair.inflow)}</span>
									</span>
									<span class="money"
										>{money(-pair.outflow.amount_minor, pair.outflow.currency)}</span
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
	</div>
</section>

<style>
	/* Clipped at the padding's edge, the button's focus ring stays whole. */
	.follows {
		position: relative;
		overflow: clip;
		align-content: start;
	}
	@media (prefers-reduced-motion: no-preference) {
		.follows {
			transition: height var(--duration-medium) var(--ease-standard);
		}
	}
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
	/* The amount ends the first line, as in every list of transactions; the pair's text wraps beside it. The line
	   spans the row (the row's other items start at its start), so the amount ends on the card's edge. */
	.line {
		justify-self: stretch;
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: baseline;
		gap: var(--space-xs) var(--space-md);
		overflow-wrap: anywhere;
	}
	.route > span {
		display: block;
	}
	.to :global(.arrow) {
		margin-inline: 0 var(--space-xs);
	}
</style>
