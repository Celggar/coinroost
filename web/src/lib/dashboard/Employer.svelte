<script lang="ts">
	// What one employer owes, from GET /payroll: the owed fortnights and their total, the ones not known yet, and
	// the history folded.
	import { scrollable } from '#lib/ui/scrollable.ts';
	import type { Schemas } from '#lib/api/client.ts';
	import { day, LIST, money, wholeDay } from '#lib/format.ts';
	import { plural, t } from '#lib/i18n.svelte.ts';
	import {
		daysText,
		fortnightList,
		fortnightName,
		historyRows,
		runParts,
		statusName,
		statusTone
	} from './payroll.ts';

	let { employer }: { employer: Schemas['EmployerPayroll'] } = $props();

	const id = $props.id();
	const c = $derived(employer.currency);
</script>

<article class="employer" aria-labelledby="{id}-name">
	<h3 id="{id}-name">{employer.name}</h3>
	{#if employer.error}
		<!-- A setting to check, not a failure of the page: the warning's amber, not the error's red. -->
		<p class="notice warn">{t('payroll.no_deposits', { date: wholeDay(employer.start) })}</p>
	{:else}
		{#if employer.owed.length}
			<p class="owed">
				<span class="label">{t('payroll.owes')}</span>
				<strong>{money(employer.owed_total_minor, c)}</strong>
				<span class="chip bad"
					>{plural(employer.owed.length, 'payroll.owed_one', 'payroll.owed_other')}</span
				>
			</p>
			<ul class="fortnights" aria-label={statusName('owed')}>
				{#each employer.owed as f (f.first_day)}
					<li>{fortnightName(f)}</li>
				{/each}
			</ul>
		{:else}
			<p class="notice">{t('payroll.owes_nothing')}</p>
		{/if}
		{#if employer.unknown.length && employer.data_until}
			<!-- The sentence says «no data» itself; a long run of fortnights is named by its ends. -->
			<p class="hint">
				{employer.unknown.length > 2
					? t('payroll.no_data_range', {
							count: employer.unknown.length,
							first: fortnightName(employer.unknown[0], true),
							last: fortnightName(employer.unknown[employer.unknown.length - 1], true),
							date: wholeDay(employer.data_until)
						})
					: t('payroll.no_data', {
							fortnights: fortnightList(employer.unknown, true),
							date: wholeDay(employer.data_until)
						})}
			</p>
		{/if}
		<p class="hint">
			{[
				plural(employer.paid, 'payroll.paid_one', 'payroll.paid_other'),
				employer.next_due &&
					t('payroll.next_due', {
						fortnight: fortnightName(employer.next_due, true),
						date: wholeDay(employer.next_due.due_on)
					})
			]
				.filter(Boolean)
				.join(LIST)}
		</p>
		{#if employer.history.length}
			<details>
				<summary>{t('payroll.history')}</summary>
				<div class="table-wrap" use:scrollable={t('payroll.history')}>
					<table class="rows-on-phone">
						<thead>
							<tr>
								<th scope="col">{t('payroll.fortnight')}</th>
								<th scope="col">{t('column.status')}</th>
								<th scope="col">{t('payroll.days')}</th>
								<th scope="col">{t('payroll.payment')}</th>
							</tr>
						</thead>
						<tbody>
							{#each historyRows(employer.history) as row (row.kind === 'line' ? row.line.fortnight.first_day : row.first.first_day)}
								{#if row.kind === 'run'}
									<tr>
										<th scope="row"
											>{plural(
												row.count,
												'payroll.fortnights_other',
												'payroll.fortnights_other'
											)}</th
										>
										<td data-label={t('column.status')}
											><span class="chip {statusTone('no_data')}">{statusName('no_data')}</span></td
										>
										<td data-label={t('payroll.days')}>
											<!-- One block, so on a phone card the range sits beside its label whole. Where the table has
											     the room the second end starts its own line, so the column is as wide as one end, not the
											     whole range on one line: that width left it half empty and broke the payment (pass 10). -->
											<span
												>{#each runParts(row.first, row.last) as part, i (i)}{#if part.end}<span
															class="end">{part.text}</span
														>{:else}{part.text}{#if i > 0}<br
																class="split"
															/>{/if}{/if}{/each}</span
											>
										</td>
										<td data-label={t('payroll.payment')}></td>
									</tr>
								{:else}
									{@const line = row.line}
									<tr>
										<th scope="row">{fortnightName(line.fortnight)}</th>
										<td data-label={t('column.status')}
											><span class="chip {statusTone(line.status)}">{statusName(line.status)}</span
											></td
										>
										<td class="when" data-label={t('payroll.days')}>{daysText(line)}</td>
										<td data-label={t('payroll.payment')}>
											<!-- One block, so on a phone card both lines sit beside the label. -->
											{#if line.payment}
												<div>
													<span class="paid"
														>{[day(line.payment.date), money(line.payment.amount_minor, c)].join(
															LIST
														)}</span
													>
													<div class="hint">
														{line.payment.description}{#if line.label}{LIST}{t('payroll.named', {
																fortnight: fortnightName(line.label, true)
															})}{/if}
													</div>
												</div>
											{/if}
										</td>
									</tr>
								{/if}
							{/each}
						</tbody>
					</table>
				</div>
			</details>
		{/if}
	{/if}
</article>

<style>
	.employer {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-sm);
		border-top: 1px solid var(--border);
		padding-top: var(--space-md);
	}
	h3 {
		margin: 0;
		font-size: var(--text-lg);
	}
	p {
		margin: 0;
	}
	.owed {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-xs) var(--space-md);
	}
	.owed strong {
		font-size: var(--text-xl);
		font-weight: 700;
	}
	.label {
		color: var(--muted);
	}
	/* The owed fortnights, one per line under their total, without the browser's bullets (no other list has them). */
	.fortnights {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: var(--space-2xs);
	}
	summary {
		cursor: pointer;
		padding: var(--space-sm) 0;
		min-height: 44px;
	}
	/* Where the table has the room, a fortnight's «Cuándo» keeps to one line and the payment, which has two, takes the
	   rest: it broke while «Pago» stood empty and wide (pass 8). A run's long range still wraps, or it takes the
	   payment's room, but only between its two ends; the payment's date and amount stay one line. Narrower, they wrap,
	   or the table would scroll sideways (900 px). */
	@media (min-width: 1100px) {
		.when,
		.paid,
		.end {
			white-space: nowrap;
		}
	}
	@media (max-width: 1099px) {
		.split {
			display: none;
		}
	}
</style>
