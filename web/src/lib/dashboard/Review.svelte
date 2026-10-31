<script lang="ts">
	// «Por revisar» (P3): what waits for the person, first on the summary, one row per kind with its count, a line of
	// detail and the way to it. A kind with nothing to do stays as a quiet «al día» line instead of vanishing (GOV.UK
	// task list: done items stay, de-emphasized). The numbers are the badges' own (`pending` in the counts store), from
	// the same queries as the pages they lead to, so the card, the badges and the pages never disagree.
	import { counts, pending } from '#lib/nav/counts.svelte.ts';
	import { LIST, list, money, wholeDay } from '#lib/format.ts';
	import { plural, t } from '#lib/i18n.svelte.ts';
	import { followContent, pop, vanish } from '#lib/motion.ts';
	import Icon from '#lib/nav/Icon.svelte';
	import type { IconName } from '#lib/nav/destinations.ts';
	import Tick from '#lib/ui/Tick.svelte';
	import type { Schemas } from '#lib/api/client.ts';

	// The page's payroll answer, when it has one: a household with employers gets a row for what they owe.
	let { payroll = null }: { payroll?: Schemas['Payroll'] | null } = $props();

	type Row = {
		key: string;
		open: boolean;
		count: number;
		icon: IconName;
		tone: 'bad' | 'warn' | 'ok';
		title: string;
		detail: string;
		href?: string;
		action?: string;
		primary?: boolean;
	};

	const transfers = $derived(pending.pairs + pending.unmatched);
	const owing = $derived(payroll?.employers.filter((e) => e.owed.length > 0) ?? []);
	// Owed money per currency, never summed across them.
	const owedTotals = $derived(
		[...new Set(owing.map((e) => e.currency))].map((currency) =>
			money(
				owing
					.filter((e) => e.currency === currency)
					.reduce((sum, e) => sum + e.owed_total_minor, 0),
				currency
			)
		)
	);
	const payrollRow = $derived<Row | null>(
		!payroll?.employers.length
			? null
			: owing.length
				? {
						key: 'payroll',
						open: true,
						count: owing.reduce((sum, e) => sum + e.owed.length, 0),
						icon: 'cash',
						tone: 'bad',
						title: t('review.owed', { amount: list(owedTotals) }),
						detail: owing
							.map((e) =>
								t('review.owed_by', {
									name: e.name,
									owed: plural(e.owed.length, 'payroll.owed_one', 'payroll.owed_other')
								})
							)
							.join(LIST),
						href: '#payroll',
						action: t('review.see')
					}
				: {
						key: 'payroll',
						open: false,
						count: 0,
						icon: 'check',
						tone: 'ok',
						title: t('payroll.title'),
						detail: t('review.nobody_owes')
					}
	);

	const rows = $derived<Row[]>([
		counts.uncategorized > 0
			? {
					key: 'uncategorized',
					open: true,
					count: counts.uncategorized,
					icon: 'tag',
					tone: 'bad',
					title: plural(
						counts.uncategorized,
						'review.uncategorized_one',
						'review.uncategorized_other'
					),
					detail: pending.spending.length
						? t('review.uncategorized_spending', {
								amount: list(pending.spending.map((s) => money(s.minor, s.currency)))
							})
						: t('review.uncategorized_no_spending'),
					href: '/uncategorized',
					action: t('review.categorize'),
					primary: true
				}
			: {
					key: 'uncategorized',
					open: false,
					count: 0,
					icon: 'check',
					tone: 'ok',
					title: t('review.transactions'),
					detail: t('review.all_categorized')
				},
		pending.late.length > 0
			? {
					key: 'late',
					open: true,
					count: pending.late.length,
					icon: 'clock',
					tone: 'warn',
					title: plural(pending.late.length, 'review.late_one', 'review.late_other'),
					detail: pending.late
						.map((a) =>
							a.latest
								? t('review.latest', { name: a.name, date: wholeDay(a.latest) })
								: t('review.no_data', { name: a.name })
						)
						.join(LIST),
					href: '/import',
					action: t('review.import')
				}
			: {
					key: 'late',
					open: false,
					count: 0,
					icon: 'check',
					tone: 'ok',
					title: t('review.accounts'),
					detail: t('review.all_current')
				},
		transfers > 0
			? {
					key: 'transfers',
					open: true,
					count: transfers,
					icon: 'transfer',
					tone: 'warn',
					title: plural(transfers, 'review.transfers_one', 'review.transfers_other'),
					detail: [
						pending.pairs && plural(pending.pairs, 'review.pairs_one', 'review.pairs_other'),
						pending.unmatched &&
							plural(pending.unmatched, 'review.unmatched_one', 'review.unmatched_other')
					]
						.filter(Boolean)
						.join(LIST),
					href: '/transfers',
					action: t('review.pair')
				}
			: {
					key: 'transfers',
					open: false,
					count: 0,
					icon: 'check',
					tone: 'ok',
					title: t('review.transfers'),
					detail: t('review.all_paired')
				},
		...(payrollRow ? [payrollRow] : [])
	]);
</script>

<section class="card review" aria-labelledby="review-title">
	<h2 id="review-title">{t('review.title')}</h2>
	<!-- The card's height follows its rows when one turns to «al día» (FD9). -->
	<div class="fit" use:followContent>
		<ul class="rows">
			{#each rows as row (row.key)}
				<!-- A row that turns «al día» fades out and the quiet line pops in, in the same place. -->
				<li class="in-place">
					{#key row.open}
						<div class="row" class:done={!row.open} in:pop={{ start: 0.96 }} out:vanish>
							<span class="badge {row.tone}"><Icon name={row.icon} size={20} /></span>
							<span class="text">
								{#if row.open}
									<strong><Tick value={row.count}>{row.title}</Tick></strong>
								{:else}
									<strong>{row.title}</strong>
								{/if}
								<span class="detail">{row.detail}</span>
							</span>
							{#if row.href && row.action}
								<a class="button" class:secondary={!row.primary} href={row.href}
									><span class="action">{row.action}</span><span class="go" aria-hidden="true"
										><Icon name="chev" size={20} /></span
									></a
								>
							{/if}
						</div>
					{/key}
				</li>
			{/each}
		</ul>
	</div>
</section>

<style>
	h2 {
		margin: 0;
	}
	.fit {
		overflow: clip;
	}
	@media (prefers-reduced-motion: no-preference) {
		.fit {
			transition: height var(--duration-medium) var(--ease-standard);
		}
	}
	.rows {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
	}
	.rows > li + li {
		border-top: 1px solid var(--border);
	}
	.row {
		display: grid;
		grid-template-columns: 40px minmax(0, 1fr) auto;
		align-items: center;
		gap: var(--space-md);
		padding: var(--space-md) 0;
	}
	.rows > li:first-child .row {
		padding-top: 0;
	}
	.rows > li:last-child .row {
		padding-bottom: 0;
	}
	.badge {
		display: grid;
		place-items: center;
		width: 40px;
		height: 40px;
		border-radius: 50%;
	}
	.badge.bad {
		background: var(--bad-bg);
		color: var(--bad);
	}
	.badge.warn {
		background: var(--warn-bg);
		color: var(--warn);
	}
	.badge.ok {
		background: var(--ok-bg);
		color: var(--ok);
	}
	/* Forced colors drop the tints: a ring keeps each mark in its circle. */
	@media (forced-colors: active) {
		.badge {
			border: 1px solid CanvasText;
		}
	}
	.text {
		display: grid;
		gap: var(--space-xs);
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.detail {
		color: var(--muted);
		font-size: var(--text-sm);
	}
	/* Done, it reads quieter than what waits: the same line, a lighter title. */
	.done strong {
		font-weight: 600;
	}
	strong {
		font-weight: 700;
	}
	.go {
		display: none;
	}
	/* A phone: a row that waits is one tap, the way «Más» lists its pages: its link shows as a chevron at the end and
	   covers the whole row (a stretched link, so its name stays «Categorizar»), with the row as its hover layer and
	   focus ring. Buttons under the words made the card fill the first screen. */
	@media (max-width: 560px) {
		.row {
			position: relative;
			isolation: isolate;
			grid-template-columns: 40px minmax(0, 1fr) auto;
			border-radius: var(--radius-button);
		}
		.row > .button {
			--state-layer: color-mix(in oklab, var(--accent) 10%, transparent);
			position: static;
			min-height: 0;
			padding: 0;
			border: 0;
			background: none;
			color: var(--muted);
		}
		.row > .button::after {
			content: '';
			position: absolute;
			inset: 0;
		}
		.row > .button:focus-visible {
			outline: none;
		}
		.row:has(> .button:focus-visible) {
			outline: 3px solid var(--focus);
			outline-offset: 2px;
		}
		.action {
			position: absolute;
			width: 1px;
			height: 1px;
			overflow: hidden;
			clip-path: inset(50%);
			white-space: nowrap;
		}
		.go {
			display: inline-flex;
		}
	}
</style>
