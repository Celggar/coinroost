// How the payroll report reads on screen.
import type { Schemas } from '#lib/api/client.ts';
import { list, month } from '#lib/format.ts';
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

/** "II quincena de marzo de 2026", "Second half of March 2026". */
export function fortnightName(f: Schemas['FortnightView']): string {
	const label = `${f.year}-${String(f.month).padStart(2, '0')}`;
	return t(f.half === 1 ? 'fortnight.1' : 'fortnight.2', { month: month(label) });
}

export function fortnightList(fortnights: Schemas['FortnightView'][]): string {
	return list(fortnights.map(fortnightName));
}

/** How late, early or overdue a fortnight is, in words; empty when there is nothing to say. */
export function daysText(line: Schemas['FortnightLine']): string {
	const days = line.days;
	if (days === null || days === undefined) return '';
	switch (line.status) {
		case 'owed':
			return plural(days, 'payroll.due_one', 'payroll.due_other');
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
