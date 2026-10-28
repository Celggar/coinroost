<script lang="ts">
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { money, today } from '#lib/format.ts';
	import { t } from '#lib/i18n.svelte.ts';

	// Reloads whenever `version` changes: pairing and splitting move money between the columns.
	let { version }: { version: number } = $props();

	let from = $state(`${today().slice(0, 4)}-01-01`);
	let to = $state(today());
	let summary = $state<Schemas['Summary'] | null>(null);
	let error = $state('');

	async function load() {
		error = '';
		if (!from || !to) return;
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
	<div class="period">
		<label>{t('period.from')} <input type="date" bind:value={from} /></label>
		<label>{t('period.to')} <input type="date" bind:value={to} /></label>
	</div>
	{#if error}<p class="alert" role="alert">{error}</p>{/if}
	{#if summary}
		{#if summary.totals.length === 0}
			<p>{t('summary.nothing')}</p>
		{:else}
			<div class="totals">
				{#each summary.totals as totals (totals.currency)}
					<dl aria-label={t('summary.totals_in', { currency: totals.currency })}>
						<div>
							<dt>{t('summary.spending')}</dt>
							<dd>{money(totals.spending, totals.currency)}</dd>
						</div>
						<div>
							<dt>{t('summary.income')}</dt>
							<dd>{money(totals.income, totals.currency)}</dd>
						</div>
						<div>
							<dt>{t('summary.unpaired_out')}</dt>
							<dd>{money(totals.unpaired_transfer_out, totals.currency)}</dd>
						</div>
						<div>
							<dt>{t('summary.unpaired_in')}</dt>
							<dd>{money(totals.unpaired_transfer_in, totals.currency)}</dd>
						</div>
					</dl>
				{/each}
			</div>
		{/if}
	{/if}
</section>

<style>
	.period {
		display: flex;
		flex-wrap: wrap;
		gap: 12px;
	}
	.totals {
		display: grid;
		gap: 12px;
	}
	dl {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 150px), 1fr));
		gap: 8px;
		margin: 0;
	}
	dt {
		color: var(--muted);
		font-size: 0.9rem;
	}
	dd {
		margin: 0;
		font-size: 1.15rem;
		font-weight: 700;
		font-variant-numeric: tabular-nums;
	}
</style>
