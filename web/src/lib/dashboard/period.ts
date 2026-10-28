// The dashboard's period and the months inside it, as YYYY-MM-DD and YYYY-MM strings.
import { today } from '#lib/format.ts';

export type Period = { from: string; to: string };
export type Preset = 'month' | 'year' | 'twelve';

const pad = (n: number) => String(n).padStart(2, '0');

/** A preset ending today: this month, this year, or the last 12 months including this one. */
export function preset(which: Preset, now: string = today()): Period {
	const [year, month] = now.split('-').map(Number);
	switch (which) {
		case 'month':
			return { from: `${now.slice(0, 7)}-01`, to: now };
		case 'year':
			return { from: `${year}-01-01`, to: now };
		case 'twelve': {
			const start = new Date(Date.UTC(year, month - 12, 1));
			return { from: `${start.getUTCFullYear()}-${pad(start.getUTCMonth() + 1)}-01`, to: now };
		}
	}
}

/** Every month from the month of `from` to the month of `to`, both included; none when `from` is after `to`. */
export function months({ from, to }: Period): string[] {
	const found: string[] = [];
	let [year, month] = from.split('-').map(Number);
	const last = to.slice(0, 7);
	for (let label = `${year}-${pad(month)}`; label <= last; label = `${year}-${pad(month)}`) {
		found.push(label);
		month += 1;
		if (month > 12) {
			month = 1;
			year += 1;
		}
	}
	return found;
}
