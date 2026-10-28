<script lang="ts">
	// What one employer owes, from GET /payroll: the owed fortnights and their total, the ones not known yet, and
	// the history folded.
	import type { Schemas } from '#lib/api/client.ts';
	import { day, money } from '#lib/format.ts';
	import { plural, t } from '#lib/i18n.svelte.ts';
	import { daysText, fortnightList, fortnightName, statusName, statusTone } from './payroll.ts';

	let { employer }: { employer: Schemas['EmployerPayroll'] } = $props();

	const id = $props.id();
	const c = $derived(employer.currency);
</script>

<article class="employer" aria-labelledby="{id}-name">
	<h3 id="{id}-name">{employer.name}</h3>
	{#if employer.error}
		<p class="alert">{t('payroll.no_deposits', { date: day(employer.start) })}</p>
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
			<p class="hint">
				<span class="chip none">{statusName('no_data')}</span>
				{t('payroll.no_data', {
					fortnights: fortnightList(employer.unknown),
					date: day(employer.data_until)
				})}
			</p>
		{/if}
		<p class="hint">
			{[
				plural(employer.paid, 'payroll.paid_one', 'payroll.paid_other'),
				employer.next_due &&
					t('payroll.next_due', {
						fortnight: fortnightName(employer.next_due),
						date: day(employer.next_due.due_on)
					})
			]
				.filter(Boolean)
				.join(' · ')}
		</p>
		{#if employer.history.length}
			<details>
				<summary>{t('payroll.history')}</summary>
				<div class="table-wrap">
					<table>
						<thead>
							<tr>
								<th scope="col">{t('payroll.fortnight')}</th>
								<th scope="col">{t('column.status')}</th>
								<th scope="col">{t('payroll.days')}</th>
								<th scope="col">{t('payroll.payment')}</th>
							</tr>
						</thead>
						<tbody>
							{#each employer.history.toReversed() as line (line.fortnight.first_day)}
								<tr>
									<td>{fortnightName(line.fortnight)}</td>
									<td
										><span class="chip {statusTone(line.status)}">{statusName(line.status)}</span
										></td
									>
									<td>{daysText(line)}</td>
									<td>
										{#if line.payment}
											{day(line.payment.date)} · {money(line.payment.amount_minor, c)}
											<div class="hint">
												{line.payment.description}{#if line.label}
													· {t('payroll.named', { fortnight: fortnightName(line.label) })}{/if}
											</div>
										{/if}
									</td>
								</tr>
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
		gap: 6px;
		border-top: 1px solid var(--border);
		padding-top: 10px;
	}
	h3 {
		margin: 0;
		font-size: 1.05rem;
	}
	p {
		margin: 0;
	}
	.owed {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 4px 10px;
	}
	.owed strong {
		font-size: 1.5rem;
		font-weight: 650;
	}
	.label {
		color: var(--muted);
	}
	.fortnights {
		margin: 0;
		padding-left: 20px;
	}
	summary {
		cursor: pointer;
		padding: 8px 0;
		min-height: 44px;
	}
</style>
