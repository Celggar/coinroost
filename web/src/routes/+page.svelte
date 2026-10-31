<script lang="ts">
	// The dashboard: what waits for the person («Por revisar», P3), what is owed to them, then the period's money per
	// currency. Every number comes from one report of the API, never summed across currencies.
	import { retry } from '#lib/ui/focus.ts';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import MonthlyChart from '#lib/dashboard/MonthlyChart.svelte';
	import Payroll from '#lib/dashboard/Payroll.svelte';
	import Review from '#lib/dashboard/Review.svelte';
	import { pending } from '#lib/nav/counts.svelte.ts';
	import { span } from '#lib/format.ts';
	import { fade, slide } from 'svelte/transition';
	import { prefersReducedMotion } from 'svelte/motion';
	import Ranking, { type Ranked } from '#lib/dashboard/Ranking.svelte';
	import Tiles from '#lib/dashboard/Tiles.svelte';
	import { months, preset, type Period, type Preset } from '#lib/dashboard/period.ts';
	import { t, type Key } from '#lib/i18n.svelte.ts';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import { DURATION, EASE, fadeThrough, pop, vanish } from '#lib/motion.ts';
	import DateRange from '#lib/ui/DateRange.svelte';

	const PAYEES = 10;
	// One segmented control for the period (P3): three presets and «Otro…», which alone shows the two dates.
	const presets: { which: Preset | 'other'; label: Key }[] = [
		{ which: 'month', label: 'dashboard.this_month' },
		{ which: 'year', label: 'dashboard.this_year' },
		{ which: 'twelve', label: 'dashboard.twelve' },
		{ which: 'other', label: 'dashboard.other' }
	];

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
	// «Otro…» chosen: the dates show and are the period, even when they happen to equal a preset.
	let custom = $state(false);
	let reports = $state<Reports | null>(null);
	let payroll = $state<Schemas['Payroll'] | null>(null);
	let payrollSettled = $state(false);
	let busy = $state(false);
	let error = $state('');
	let latest = 0;
	let shown = $state<HTMLElement>();
	// Answers are shown one at a time, each with a fade through (FD9): two quick changes of period never cross.
	let showing = Promise.resolve();
	const datesOk = $derived(!!from && !!to && from <= to);

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

	function choose(which: Preset | 'other') {
		if (which === 'other') {
			custom = true;
			return;
		}
		custom = false;
		({ from, to } = preset(which));
	}

	function chosen(which: Preset | 'other'): boolean {
		if (which === 'other') return custom;
		const range = preset(which);
		return !custom && from === range.from && to === range.to;
	}

	// The dates open under the control and close again (FD9); under reduced motion they only fade.
	const open = (node: Element) =>
		prefersReducedMotion.current
			? fade(node, { duration: DURATION.short })
			: slide(node, { duration: DURATION.medium, easing: EASE.enter });
	const close = (node: Element) =>
		prefersReducedMotion.current
			? fade(node, { duration: DURATION.short })
			: slide(node, { duration: DURATION.short, easing: EASE.exit });

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
		 later pushed the period's control down under the pointer. A household with no account gets the empty card. -->
	{#if ready && reports}
		<div class="stack" in:fade={{ duration: DURATION.medium }}>
			{#if reports.hasAccounts && pending.ready}
				<Review {payroll} />
			{/if}

			<!-- The period only where there is money to pick from: with no account yet, the page is its way on (pass 7). -->
			{#if reports.hasAccounts}
				<!-- The period (P3): one segmented control under its label, the range it stands for beside it; the two dates
				 only under «Otro…». The control's segments share its width, so a check coming or going moves nothing else. -->
				<div class="period">
					<div class="form-field" role="group" aria-labelledby="period-label">
						<span class="label" id="period-label">{t('dashboard.period')}</span>
						<div class="presets">
							{#each presets as option (option.which)}
								{@const on = chosen(option.which)}
								<button
									class="secondary"
									aria-pressed={on}
									aria-controls={option.which === 'other' ? 'period-dates' : undefined}
									onclick={() => choose(option.which)}
									>{#if on}<svg
											class="check"
											viewBox="0 0 24 24"
											aria-hidden="true"
											in:pop
											out:vanish><path d="M5 12.5l4.5 4.5L19 7.5" /></svg
										>{/if}{t(option.label)}</button
								>
							{/each}
						</div>
					</div>
					{#if !custom && datesOk}
						<p class="range" in:fade={{ duration: DURATION.short }}>{span(from, to)}</p>
					{/if}
				</div>
				{#if custom}
					<div class="dates" id="period-dates" in:open out:close>
						<DateRange bind:from bind:to />
					</div>
				{/if}
			{/if}

			{#if error}
				<p class="alert" role="alert">
					{error}
					<button class="secondary" onclick={() => retry(() => load({ from, to }))}
						>{t('app.try_again')}</button
					>
				</p>
			{/if}

			<div class="reports stack" class:stale={busy || !datesOk} bind:this={shown}>
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
		<p class="alert" role="alert">
			{error}
			<button
				class="secondary"
				onclick={(event) => retry(event.currentTarget, () => load({ from, to }))}
				>{t('app.try_again')}</button
			>
		</p>
	{:else}
		<p class="hint loading">{t('app.loading')}</p>
	{/if}
	<!-- What is owed to the person, after the period's figures: «Por revisar» above says who owes and leads here. -->
	<Payroll bind:payroll bind:settled={payrollSettled} />
</div>

<style>
	.period {
		display: flex;
		flex-wrap: wrap;
		align-items: start;
		gap: var(--space-md) var(--space-lg);
	}
	/* Material 3 segmented buttons: joined segments, the chosen one filled. Each segment has room for its label and
	   the check, so the check coming or going never moves the dates beside them. */
	.presets {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		width: min(100%, 37rem);
	}
	/* The range the presets stand for, level with the control's middle (not its label) when beside it; under it on
	   a narrower page, without a band of empty height. */
	.range {
		align-self: end;
		margin: 0;
		padding-bottom: calc((var(--field-height) - 1lh) / 2);
		color: var(--muted);
	}
	.presets button {
		white-space: nowrap;
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
	.label {
		font-weight: 600;
		line-height: 1.25;
	}
	/* Material 3: the chosen segment is filled and carries a check; the outline stays the group's. */
	.presets button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-xs);
	}
	.presets button[aria-pressed='true'] {
		background: color-mix(in oklab, var(--accent) 22%, var(--surface));
		color: var(--text);
	}
	/* Forced colors drop the fill: the chosen segment takes the system's selection colors, as a selected item does,
	   so it shows on a phone too, where it has no check. */
	@media (forced-colors: active) {
		.presets button[aria-pressed='true'] {
			forced-color-adjust: none;
			background: Highlight;
			color: HighlightText;
			border-color: Highlight;
		}
	}
	.check {
		flex: none;
		width: 18px;
		height: 18px;
		fill: none;
		stroke: currentColor;
		stroke-width: 2.5;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	.period > .form-field {
		flex: 1 1 auto;
		max-width: 37rem;
	}
	/* A phone: the four segments across the width with a smaller label, the range under them. Each segment takes
	   its label's width plus an equal share of the rest (UIKit's segments «apportioned by content»): equal ones left
	   «This month» and «12 meses» touching their borders. The chosen one shows by its fill alone, as Material's
	   segmented button allows: the check needs room four segments do not have there. */
	@media (max-width: 560px) {
		.period > .form-field,
		.presets {
			width: 100%;
			max-width: none;
		}
		.range {
			padding-bottom: 0;
		}
		.presets {
			display: flex;
		}
		.presets button {
			flex: 1 1 auto;
			padding: 0 var(--space-sm);
			font-size: var(--text-sm);
		}
		.check {
			display: none;
		}
	}
	@media (max-width: 359px) {
		.presets button {
			padding: 0 var(--space-xs);
			font-size: var(--text-xs);
		}
	}
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
		transition: opacity 0.2s;
	}
	.stale {
		opacity: 0.6;
	}
	h2 {
		margin: 0;
	}
</style>
