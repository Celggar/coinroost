<script lang="ts">
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { money, today } from '#lib/format.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import DateRange from '#lib/ui/DateRange.svelte';
	import Tick from '#lib/ui/Tick.svelte';

	// Reloads whenever `version` changes: pairing and splitting move money between the columns.
	let { version }: { version: number } = $props();

	let from = $state(`${today().slice(0, 4)}-01-01`);
	let to = $state(today());
	let summary = $state<Schemas['Summary'] | null>(null);
	let error = $state('');
	// An end before the start asks nothing: the last figures stay, dimmed, while the field says what is wrong.
	const datesOk = $derived(!!from && !!to && from <= to);

	async function load() {
		error = '';
		if (!datesOk) return;
		const {
			data,
			error: failure,
			response
		} = await api.GET('/api/v1/summary', { params: { query: { from, to } } });
		if (!data) {
			summary = null;
			error = problem(failure, response?.status, t('summary.failed'));
			return;
		}
		summary = data;
	}

	$effect(() => {
		void version;
		void from;
		void to;
		load();
	});
</script>

<section class="card" aria-labelledby="summary-title">
	<h2 id="summary-title">{t('summary.title')}</h2>
	<p class="hint">{t('summary.hint')}</p>
	<DateRange bind:from bind:to />
	{#if error}<p class="alert" role="alert">{error}</p>{/if}
	{#if summary}
		{#if summary.totals.length === 0}
			<p>{t('summary.nothing')}</p>
		{:else}
			<div class="totals" class:stale={!datesOk}>
				{#each summary.totals as totals (totals.currency)}
					<dl class="figures" aria-label={t('summary.totals_in', { currency: totals.currency })}>
						<div>
							<dt>{t('summary.spending')}</dt>
							<dd class="value">
								<Tick value={totals.spending}>{money(totals.spending, totals.currency)}</Tick>
							</dd>
						</div>
						<div>
							<dt>{t('summary.income')}</dt>
							<dd class="value">
								<Tick value={totals.income}>{money(totals.income, totals.currency)}</Tick>
							</dd>
						</div>
						<div>
							<dt>{t('summary.unpaired_out')}</dt>
							<dd class="value">
								<Tick value={totals.unpaired_transfer_out}
									>{money(totals.unpaired_transfer_out, totals.currency)}</Tick
								>
							</dd>
						</div>
						<div>
							<dt>{t('summary.unpaired_in')}</dt>
							<dd class="value">
								<Tick value={totals.unpaired_transfer_in}
									>{money(totals.unpaired_transfer_in, totals.currency)}</Tick
								>
							</dd>
						</div>
					</dl>
				{/each}
			</div>
		{/if}
	{/if}
</section>

<style>
	.totals {
		display: grid;
		gap: var(--space-md);
		transition: opacity var(--duration-short, 150ms);
	}
	.stale {
		opacity: 0.5;
	}
</style>
