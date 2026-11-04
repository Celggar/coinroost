<script lang="ts">
	// The dashboard: what waits for the person («Por revisar», P3), what is owed to them, then the period's money per
	// currency. Every number comes from one report of the API, never summed across currencies.
	import { retry } from '#lib/ui/focus.ts';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import MonthlyChart from '#lib/dashboard/MonthlyChart.svelte';
	import Payroll from '#lib/dashboard/Payroll.svelte';
	import Review from '#lib/dashboard/Review.svelte';
	import { pending } from '#lib/nav/counts.svelte.ts';
	import Ranking, { type Ranked } from '#lib/dashboard/Ranking.svelte';
	import Tiles from '#lib/dashboard/Tiles.svelte';
	import { months, preset, type Period } from '#lib/dashboard/period.ts';
	import PeriodPicker from '#lib/dashboard/PeriodPicker.svelte';
	import { t, type Key } from '#lib/i18n.svelte.ts';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import { arrive, fadeThrough, lift, placeholder, rise } from '#lib/motion.ts';

	const PAYEES = 10;
	type Reports = {
		summary: Schemas['Summary'];
		monthly: Schemas['MonthTotals'][];
		categories: Schemas['CategorySpending'][];
		payees: Schemas['PayeeSpending'][];
		categoryNames: Map<string, string>;
		payeeNames: Map<string, string>;
		hasAccounts: boolean;
		/** An account a bank's file imports into: with only cash accounts, importing is not the way on. */
		hasBankFile: boolean;
		period: Period;
	};

	let from = $state(preset('year').from);
	let to = $state(preset('year').to);
	let reports = $state<Reports | null>(null);
	let payroll = $state<Schemas['Payroll'] | null>(null);
	let payrollSettled = $state(false);
	let busy = $state(false);
	let error = $state('');
	let latest = 0;
	let shown = $state<HTMLElement>();
	// Answers are shown one at a time, each with a fade through (FD9): two quick changes of period never cross.
	let showing = Promise.resolve();

	async function load(period: Period) {
		// An end before the start asks nothing: the last answer stays as it is while the field says what is wrong.
		// Dimmed for that while, its labels fell under 4.5:1 for as long as the dates stayed wrong (WCAG 1.4.3).
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
			api.GET('/api/v1/payees'),
			api.GET('/api/v1/accounts')
		] as const);
		if (ticket !== latest) return;
		const [summary, monthly, categories, payees, categoryList, payeeList, accountList] = answers;
		if (
			!summary.data ||
			!monthly.data ||
			!categories.data ||
			!payees.data ||
			!categoryList.data ||
			!payeeList.data ||
			!accountList.data
		) {
			const failed = answers.find((a) => !a.data);
			error = problem(failed?.error, failed?.response?.status, t('dashboard.failed'));
			busy = false;
			return;
		}
		const next: Reports = {
			summary: summary.data,
			monthly: monthly.data.months,
			categories: categories.data.items,
			payees: payees.data.items,
			categoryNames: new Map(categoryList.data.items.map((c) => [c.id, c.name])),
			payeeNames: new Map(payeeList.data.items.map((p) => [p.id, p.name])),
			hasAccounts: accountList.data.items.length > 0,
			hasBankFile: accountList.data.items.some((a) => a.profile),
			period
		};
		showing = showing.then(() => {
			// A newer answer is on its way: it is the one to show.
			if (ticket !== latest) return;
			// The figures stay dimmed until they fade out: lit up first, they flashed before the fade (P3 round 2).
			return fadeThrough(reports ? shown : undefined, () => {
				reports = next;
				busy = false;
			});
		});
	}

	$effect(() => {
		load({ from, to });
	});

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

	const ready = $derived(!!reports && (pending.ready || pending.failed) && payrollSettled);
	const currencies = $derived(reports?.summary.totals ?? []);
	const monthList = $derived(reports ? months(reports.period) : []);
</script>

<svelte:head>
	<title>{t('app.title', { page: t('dashboard.title') })}</title>
</svelte:head>

<h1>{t('dashboard.title')}</h1>

<div class="stack">
	<!-- The top shows whole, once the reports, the counts and the payroll have answered: a card or a row arriving
		 later pushed the period's control down under the pointer. A household with no account gets the empty card, and one
		 with no data yet goes without «Por revisar»: three green lines over «import a statement» said all was done (pass 29). -->
	{#if ready && reports}
		<div class="stack" in:arrive>
			{#if reports.hasAccounts && pending.ready && pending.started}
				<Review {payroll} />
			{/if}

			<!-- The period only where there is money to pick from: with no account yet, the page is its way on (pass 7). -->
			{#if reports.hasAccounts}
				<PeriodPicker bind:from bind:to shown={reports.period} stale={busy} />
			{/if}

			{#if error}
				<p class="alert" role="alert" in:rise out:lift>
					{error}
					<button
						class="secondary"
						onclick={(event) => retry(event.currentTarget, () => load({ from, to }))}
						>{t('app.try_again')}</button
					>
				</p>
			{/if}

			<div class="reports stack" class:stale={busy} bind:this={shown}>
				{#if currencies.length === 0}
					<!-- The way on from an empty summary, as a card like the import page's zone, not a lone sentence over an
					     empty page: a household with no account yet makes one first; one with accounts imports a statement. -->
					{#if reports.hasBankFile}
						<EmptyState
							icon="import"
							text={t('dashboard.nothing')}
							href="/import"
							action={t('dashboard.to_import')}
						/>
					{:else if reports.hasAccounts}
						<!-- Only cash accounts: Import would say there is no account for a bank's file (pass 7). -->
						<EmptyState
							icon="bank"
							text={`${t('dashboard.nothing')} ${t('import.no_bank_file')}`}
							href="/accounts"
							action={t('file.open_accounts')}
						/>
					{:else}
						<EmptyState
							icon="bank"
							text={t('dashboard.no_accounts')}
							href="/accounts"
							action={t('dashboard.to_accounts')}
						/>
					{/if}
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
								hint={t('ranking.categories_hint')}
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
		</div>
	{:else if error}
		<p class="alert" role="alert" in:arrive out:placeholder>
			{error}
			<button
				class="secondary"
				onclick={(event) => retry(event.currentTarget, () => load({ from, to }))}
				>{t('app.try_again')}</button
			>
		</p>
	{:else}
		<p class="hint loading" out:placeholder>{t('app.loading')}</p>
	{/if}
	<!-- What is owed to the person, after the period's figures: «Por revisar» above says who owes and leads here. -->
	<Payroll bind:payroll bind:settled={payrollSettled} />
</div>

<style>
	/* Side by side the two cards share their height and their rows: each has a title and a hint line (pass 1). */
	.rankings {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 320px), 1fr));
		gap: var(--space-md);
	}
	.rankings > :global(.card) {
		align-content: start;
	}
	/* While another period loads, the last one stays in place, dimmed, so nothing jumps. The transition lives on the
	   block, not on the dimmed state: there, taking the class away put the figures back to full in one frame. */
	.reports {
		transition: opacity var(--duration-exit) var(--ease-standard);
	}
	.stale {
		opacity: 0.6;
	}
	h2 {
		margin: 0;
	}
</style>
