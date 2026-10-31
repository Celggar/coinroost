// How the payroll report reads on screen.
import type { Schemas } from '#lib/api/client.ts';
import { list, month, wholeDay } from '#lib/format.ts';
import { plural, t, type Key } from '#lib/i18n.svelte.ts';

type Status = Schemas['FortnightStatus'];

// A new status in the contract fails the type check until it has a name and a tone here.
const statusKeys = {
	owed: 'payroll.status.owed',
	no_data: 'payroll.status.no_data',
	late: 'payroll.status.late',
	on_time: 'payroll.status.on_time',
	ahead: 'payroll.status.ahead'
} satisfies Record<Status, Key>;

const tones = {
	owed: 'bad',
	no_data: 'none',
	late: 'warn',
	on_time: 'ok',
	ahead: 'ok'
} satisfies Record<Status, string>;

export const statusName = (status: Status) => t(statusKeys[status]);
export const statusTone = (status: Status) => tones[status];

/** "II quincena de marzo de 2026", "Second half of March 2026"; `inline` inside a sentence, where English goes
 * lower case («… for second half of May 2026», pass 7) and the Roman numeral stays. */
export function fortnightName(f: Schemas['FortnightView'], inline = false): string {
	const label = `${f.year}-${String(f.month).padStart(2, '0')}`;
	const key = inline
		? f.half === 1
			? 'fortnight.1_inline'
			: 'fortnight.2_inline'
		: f.half === 1
			? 'fortnight.1'
			: 'fortnight.2';
	return t(key, { month: month(label) });
}

export function fortnightList(fortnights: Schemas['FortnightView'][], inline = false): string {
	return list(fortnights.map((f) => fortnightName(f, inline)));
}

/**
 * A run of fortnights, «{first} a {last}», in parts with its two ends marked, so the page can keep each end whole and
 * break only between them (it broke inside «I quincena de octubre», pass 8).
 */
export function runParts(
	first: Schemas['FortnightView'],
	last: Schemas['FortnightView']
): { text: string; end: boolean }[] {
	const ends = [fortnightName(first), fortnightName(last, true)];
	return t('payroll.run', { first: '\u0000', last: '\u0001' })
		.split(/([\u0000\u0001])/)
		.filter(Boolean)
		.map((part) =>
			part === '\u0000' || part === '\u0001'
				? { text: ends[part.charCodeAt(0)], end: true }
				: { text: part, end: false }
		);
}

/** How late, early or overdue a fortnight is, in words; empty when there is nothing to say. */
export function daysText(line: Schemas['FortnightLine']): string {
	const days = line.days;
	if (days === null || days === undefined) return '';
	switch (line.status) {
		// Owed days are counted to the account's last data, not to today: the date it fell due says it without a
		// count that read as recent five months later (pass 7).
		case 'owed':
			return t('payroll.was_due', { date: wholeDay(line.fortnight.due_on) });
		case 'late':
			return plural(days, 'payroll.late_one', 'payroll.late_other');
		case 'on_time':
		case 'ahead':
			return days === 0
				? t('payroll.on_the_day')
				: plural(-days, 'payroll.early_one', 'payroll.early_other');
		default:
			return '';
	}
}

export type HistoryRow =
	| { kind: 'line'; line: Schemas['FortnightLine'] }
	| { kind: 'run'; first: Schemas['FortnightView']; last: Schemas['FortnightView']; count: number };

/** The history, newest first, with three or more fortnights in a row without data as one row: on a phone ten cards
 * that only said «Sin datos» came before the ones that matter (pass 7). */
export function historyRows(history: Schemas['FortnightLine'][]): HistoryRow[] {
	const rows: HistoryRow[] = [];
	let run: Schemas['FortnightLine'][] = [];
	const flush = () => {
		if (run.length >= 3) {
			rows.push({
				kind: 'run',
				first: run[run.length - 1].fortnight,
				last: run[0].fortnight,
				count: run.length
			});
		} else {
			rows.push(...run.map((line) => ({ kind: 'line' as const, line })));
		}
		run = [];
	};
	for (const line of history.toReversed()) {
		if (line.status === 'no_data') {
			run.push(line);
			continue;
		}
		flush();
		rows.push({ kind: 'line', line });
	}
	flush();
	return rows;
}
