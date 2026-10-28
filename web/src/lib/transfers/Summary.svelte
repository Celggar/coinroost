<script lang="ts">
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { money, today } from '#lib/format.ts';

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
			error = problem(failure, response?.status, 'The summary could not be loaded');
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
	<h2 id="summary-title">Money in and out</h2>
	<p class="hint">
		Transfers between your accounts count in neither. Legs still waiting for their other side are
		shown apart.
	</p>
	<div class="period">
		<label>From <input type="date" bind:value={from} /></label>
		<label>To <input type="date" bind:value={to} /></label>
	</div>
	{#if error}<p class="alert" role="alert">{error}</p>{/if}
	{#if summary}
		{#if summary.totals.length === 0}
			<p>No money moved in this period.</p>
		{:else}
			<div class="totals">
				{#each summary.totals as totals (totals.currency)}
					<dl aria-label={`Totals in ${totals.currency}`}>
						<div>
							<dt>Spending</dt>
							<dd>{money(totals.spending, totals.currency)}</dd>
						</div>
						<div>
							<dt>Income</dt>
							<dd>{money(totals.income, totals.currency)}</dd>
						</div>
						<div>
							<dt>Transfers out, unpaired</dt>
							<dd>{money(totals.unpaired_transfer_out, totals.currency)}</dd>
						</div>
						<div>
							<dt>Transfers in, unpaired</dt>
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
