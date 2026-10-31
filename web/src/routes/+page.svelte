<script lang="ts">
	// The dashboard: what is owed to the person, then the period's money per currency. Every number comes from
	// one report of the API, never summed across currencies.
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import MonthlyChart from '#lib/dashboard/MonthlyChart.svelte';
	import Payroll from '#lib/dashboard/Payroll.svelte';
	import Ranking, { type Ranked } from '#lib/dashboard/Ranking.svelte';
	import Tiles from '#lib/dashboard/Tiles.svelte';
	import { months, preset, type Period, type Preset } from '#lib/dashboard/period.ts';
	import { t, type Key } from '#lib/i18n.svelte.ts';

	const PAYEES = 10;
	const presets: { which: Preset; label: Key }[] = [
		{ which: 'month', label: 'dashboard.this_month' },
		{ which: 'year', label: 'dashboard.this_year' },
		{ which: 'twelve', label: 'dashboard.twelve' }
	];

	type Reports = {
		summary: Schemas['Summary'];
		monthly: Schemas['MonthTotals'][];
		categories: Schemas['CategorySpending'][];
		payees: Schemas['PayeeSpending'][];
		categoryNames: Map<string, string>;
		payeeNames: Map<string, string>;
		period: Period;
	};

	let from = $state(preset('year').from);
	let to = $state(preset('year').to);
	let reports = $state<Reports | null>(null);
	let busy = $state(false);
	let error = $state('');
	let latest = 0;

	async function load(period: Period) {
		if (!period.from || !period.to || period.from > period.to) return;
		// Only the answer to the latest request is shown, whatever order they come back in.
		const ticket = ++latest;
		busy = true;
		error = '';
		const query = { from: period.from, to: period.to };
		const answers = await Promise.all([
			api.GET('/api/v1/summary', { params: { query } }),
			api.GET('/api/v1/reports/monthly', { params: { query } }),
			api.GET('/api/v1/reports/categories', { params: { query } }),
			api.GET('/api/v1/reports/payees', { params: { query: { ...query, limit: PAYEES } } }),
			api.GET('/api/v1/categories'),
			api.GET('/api/v1/payees')
		] as const);
		if (ticket !== latest) return;
		busy = false;
		const [summary, monthly, categories, payees, categoryList, payeeList] = answers;
		if (
			!summary.data ||
			!monthly.data ||
			!categories.data ||
			!payees.data ||
			!categoryList.data ||
			!payeeList.data
		) {
			const failed = answers.find((a) => !a.data);
			error = problem(failed?.error, failed?.response?.status, t('dashboard.failed'));
			return;
		}
		reports = {
			summary: summary.data,
			monthly: monthly.data.months,
			categories: categories.data.items,
			payees: payees.data.items,
			categoryNames: new Map(categoryList.data.items.map((c) => [c.id, c.name])),
			payeeNames: new Map(payeeList.data.items.map((p) => [p.id, p.name])),
			period
		};
	}

	$effect(() => {
		load({ from, to });
	});

	function choose(which: Preset) {
		({ from, to } = preset(which));
	}

	function ranked(
		items: { id: string | null | undefined; spending: number; count: number }[],
		names: Map<string, string>,
		none: Key
	): Ranked[] {
		return items.map((item) => ({
			key: item.id ?? '',
			name: item.id ? (names.get(item.id) ?? '?') : t(none),
			spending: item.spending,
			count: item.count
		}));
	}

	const currencies = $derived(reports?.summary.totals ?? []);
	const monthList = $derived(reports ? months(reports.period) : []);
</script>

<svelte:head>
	<title>{t('app.title', { page: t('dashboard.title') })}</title>
</svelte:head>

<h1>{t('dashboard.title')}</h1>

<div class="stack">
	<Payroll />

	<!-- The presets are one segmented control, as tall as the date fields beside them, so every control of the row
		 shares its top and bottom edges (his #158); the two dates sit in a grid of their own. -->
	<div class="period" role="group" aria-label={t('dashboard.period')}>
		<div class="presets">
			{#each presets as option (option.which)}
				{@const range = preset(option.which)}
				<button
					class="secondary"
					aria-pressed={from === range.from && to === range.to}
					onclick={() => choose(option.which)}>{t(option.label)}</button
				>
			{/each}
		</div>
		<div class="date-range">
			<label>{t('period.from')} <input type="date" bind:value={from} /></label>
			<label>{t('period.to')} <input type="date" bind:value={to} /></label>
		</div>
	</div>

	{#if error}
		<p class="alert" role="alert">
			{error}
			<button class="secondary" onclick={() => load({ from, to })}>{t('app.try_again')}</button>
		</p>
	{/if}

	{#if reports}
		<div class="reports stack" class:stale={busy}>
			{#if currencies.length === 0}
				<p>{t('dashboard.nothing')}</p>
			{/if}
			{#each currencies as totals (totals.currency)}
				{@const c = totals.currency}
				<section class="stack" aria-label={t('dashboard.in_currency', { currency: c })}>
					{#if currencies.length > 1}<h2>{t('dashboard.in_currency', { currency: c })}</h2>{/if}
					<Tiles {totals} />
					<div class="card">
						<MonthlyChart
							rows={reports.monthly.filter((r) => r.currency === c)}
							months={monthList}
							currency={c}
						/>
					</div>
					<div class="rankings">
						<Ranking
							title={t('ranking.categories')}
							currency={c}
							items={ranked(
								reports.categories
									.filter((i) => i.currency === c)
									.map((i) => ({ ...i, id: i.category_id })),
								reports.categoryNames,
								'ranking.no_category'
							)}
						/>
						<Ranking
							title={t('ranking.payees')}
							hint={t('ranking.payees_hint', { count: PAYEES })}
							currency={c}
							shown={PAYEES}
							items={ranked(
								reports.payees
									.filter((i) => i.currency === c)
									.map((i) => ({ ...i, id: i.payee_id })),
								reports.payeeNames,
								'ranking.no_payee'
							)}
						/>
					</div>
				</section>
			{/each}
		</div>
	{:else if !error}
		<p class="hint">{t('app.loading')}</p>
	{/if}
</div>

<style>
	.period {
		display: flex;
		flex-wrap: wrap;
		align-items: end;
		gap: var(--space-md) var(--space-lg);
	}
	/* Material 3 segmented buttons: joined segments, the chosen one filled. */
	.presets {
		display: inline-flex;
	}
	.presets button {
		min-height: var(--field-height);
		border-radius: 0;
		padding: 0 var(--space-lg);
	}
	/* Neighbours share one border line; the chosen one draws its own all round, inside. */
	.presets button + button {
		border-left-width: 0;
	}
	.presets button:first-child {
		border-radius: var(--radius-field) 0 0 var(--radius-field);
	}
	.presets button:last-child {
		border-radius: 0 var(--radius-field) var(--radius-field) 0;
	}
	.presets button[aria-pressed='true'] {
		background: color-mix(in oklab, var(--accent) 14%, var(--surface));
		color: var(--text);
		box-shadow: inset 0 0 0 1px var(--text);
	}
	/* A phone: the presets in three equal segments across, the two dates side by side under them. */
	@media (max-width: 560px) {
		.presets,
		.date-range {
			display: grid;
			width: 100%;
		}
		.presets {
			grid-template-columns: repeat(3, minmax(0, 1fr));
		}
		.presets button {
			padding: 0 var(--space-sm);
		}
	}
	.rankings {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 320px), 1fr));
		gap: var(--space-md);
		align-items: start;
	}
	/* While another period loads, the last one stays in place, dimmed, so nothing jumps. */
	.stale {
		opacity: 0.6;
		transition: opacity 0.2s;
	}
	h2 {
		margin: 0;
	}
</style>
