<script lang="ts">
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { money, today } from '#lib/format.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import DateRange from '#lib/ui/DateRange.svelte';
	import Tick from '#lib/ui/Tick.svelte';
	import { retry } from '#lib/ui/focus.ts';
	import { lift, rise, vanish } from '#lib/motion.ts';

	// Reloads whenever `version` changes: pairing and splitting move money between the columns.
	let { version }: { version: number } = $props();

	let from = $state(`${today().slice(0, 4)}-01-01`);
	let to = $state(today());
	let summary = $state<Schemas['Summary'] | null>(null);
	let error = $state('');
	// An end before the start asks nothing: the last figures stay as they are while the field says what is wrong.
	// Dimmed for that while, their labels fell under 4.5:1 for as long as the dates stayed wrong (WCAG 1.4.3).
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
	<!-- Its own way back, as every failed load has: the message says to try again. -->
	{#if error}
		<p class="alert" role="alert" in:rise out:lift>
			{error}
			<button
				class="secondary"
				onclick={(event) => retry(event.currentTarget, load, '#summary-title')}
				>{t('app.try_again')}</button
			>
		</p>
	{/if}
	<!-- One block for either shape: the figures rise in when they load and fade out when a load fails. -->
	{#if summary}
		<div in:rise out:vanish>
			{#if summary.totals.length === 0}
				<p>{t('summary.nothing')}</p>
			{:else}
				<div class="totals">
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
		</div>
	{/if}
</section>

<style>
	.totals {
		display: grid;
		gap: var(--space-md);
	}
</style>
