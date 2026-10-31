<script lang="ts">
	// «Por revisar» (P3): what waits for the person, first on the summary, one row per kind with its count, a line of
	// detail and the way to it. A kind with nothing to do stays as a quiet «al día» line instead of vanishing (GOV.UK
	// task list: done items stay, de-emphasized). The numbers are the badges' own (`pending` in the counts store), from
	// the same queries as the pages they lead to, so the card, the badges and the pages never disagree.
	import { counts, pending } from '#lib/nav/counts.svelte.ts';
	import { LIST, list, money, wholeDay } from '#lib/format.ts';
	import { plural, t } from '#lib/i18n.svelte.ts';
	import { DURATION, followContent, pop, vanish } from '#lib/motion.ts';
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

	// The leaving row's place is held while it fades out (its height on the item), then given up.
	function leave(event: Event) {
		const row = event.currentTarget as HTMLElement;
		const item = row.parentElement;
		if (item) item.style.minHeight = `${row.offsetHeight}px`;
		row.classList.add('leaving');
	}
	function left(event: Event) {
		const item = (event.currentTarget as HTMLElement).parentElement;
		if (item) item.style.minHeight = '';
	}

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
				<!-- A row that turns «al día» fades out, then the quiet line pops in, in the same place: crossing, the two
				     texts read on top of each other (P3 round 2). -->
				<!-- `data-flip`: when a row changes height the rows under it glide to their places with the card, not
				     in one frame (P3 round 2: 24 px). The row that leaves fades out of the flow but keeps its place
				     until it is gone, and only then the rows glide: sliding up at once, the next row passed under
				     its last line while it still showed (P3 round 3). -->
				<li class="in-place" data-flip>
					{#key row.open}
						<div
							class="row"
							class:done={!row.open}
							in:pop={{ start: 0.96, delay: DURATION.short }}
							out:vanish
							onoutrostart={leave}
							onoutroend={left}
						>
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
	/* The card clips its rows only while its height glides: at rest the focus ring, 5 px outside a row, shows whole
	   (it lost its sides on the phone and its right edge on the desktop, P3 round 1). */
	.fit:global([data-moving]) {
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
	.rows > li {
		position: relative;
	}
	.rows > li + li {
		border-top: 1px solid var(--border);
	}
	/* Its padding in two variables: the phone's focus ring is drawn from the content, whatever the padding. */
	.row {
		--pad-top: var(--space-md);
		--pad-bottom: var(--space-md);
		display: grid;
		grid-template-columns: 40px minmax(0, 1fr) auto;
		align-items: center;
		gap: var(--space-md);
		padding: var(--pad-top) 0 var(--pad-bottom);
	}
	.rows > li:first-child .row {
		--pad-top: 0px;
	}
	.rows > li:last-child .row {
		--pad-bottom: 0px;
	}
	.row:global(.leaving) {
		position: absolute;
		inset: 0 0 auto;
	}
	/* The card's buttons share one height, as on the board: the primary's 48 px stood out of the column (P3 round 2). */
	.row > .button {
		min-height: 44px;
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
			grid-template-columns: 40px minmax(0, 1fr) 20px;
			border-radius: var(--radius-button);
		}
		/* The link itself covers the row, its chevron at the end over the last column, so its box is its tap area
		   (an ::after did it, and a box of 20 px read as a target too small). */
		.row > .button {
			--state-layer: color-mix(in oklab, var(--accent) 10%, transparent);
			position: absolute;
			inset: 0;
			display: flex;
			justify-content: flex-end;
			align-items: center;
			min-height: 0;
			/* The row's own padding, so the chevron sits on the words' middle in the first and last rows too. */
			padding: inherit;
			border: 0;
			border-radius: inherit;
			background: none;
			color: var(--muted);
		}
		.row > .button:focus-visible {
			outline: none;
		}
		/* A press does not scale the link: it is the whole row, and the row stays put, as «Más»'s do. (When its tap
		   area was an ::after, the scale shrank that area to the chevron and a tap on the words opened nothing, P3
		   round 1.) */
		.row > .button:active {
			transform: none;
		}
		/* The ring 8 px around the row's content on every side, so it is even in the first and last rows (no padding
		   at their outer edge) and the badge has the chevron's air; it stays 8 px clear of the lines between rows. */
		.row:has(> .button:focus-visible)::before {
			content: '';
			position: absolute;
			inset: calc(var(--pad-top) - var(--space-sm)) calc(-1 * var(--space-sm))
				calc(var(--pad-bottom) - var(--space-sm));
			border: 3px solid var(--focus);
			border-radius: var(--radius-button);
			pointer-events: none;
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
