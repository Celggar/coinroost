// How amounts and dates are shown, in the current language.
import { tag } from '#lib/i18n.svelte.ts';

/** Minor units as an amount in the currency, such as -4599 USD → -$45.99. */
export function money(minor: number, currency: string): string {
	// Narrow symbols: es-PA would otherwise write "USD 45.99".
	const format = new Intl.NumberFormat(tag(), {
		style: 'currency',
		currency,
		currencyDisplay: 'narrowSymbol'
	});
	const digits = format.resolvedOptions().maximumFractionDigits ?? 2;
	return minus(format.format(minor / 10 ** digits));
}

/** The minus sign (U+2212), as wide as the plus, instead of the hyphen Intl writes for some languages. */
function minus(text: string): string {
	return text.replace('-', '\u2212');
}

/** A movement of money with its sign always written: +$1,462.16, −$45.50 (zero has none). */
export function signed(minor: number, currency: string): string {
	const format = new Intl.NumberFormat(tag(), {
		style: 'currency',
		currency,
		currencyDisplay: 'narrowSymbol',
		signDisplay: 'exceptZero'
	});
	const digits = format.resolvedOptions().maximumFractionDigits ?? 2;
	return minus(format.format(minor / 10 ** digits));
}

function utc(iso: string): Date {
	const [year, month, day] = iso.split('-').map(Number);
	return new Date(Date.UTC(year, month - 1, day));
}

/** A YYYY-MM-DD date as people read it: "15 may 2026", "May 15, 2026". */
export function day(iso: string): string {
	return new Intl.DateTimeFormat(tag(), {
		day: 'numeric',
		month: 'short',
		year: 'numeric',
		timeZone: 'UTC'
	}).format(utc(iso));
}

/** A YYYY-MM month: "mayo de 2026", "May 2026". */
export function month(yearMonth: string): string {
	return new Intl.DateTimeFormat(tag(), { month: 'long', year: 'numeric', timeZone: 'UTC' }).format(
		utc(`${yearMonth}-01`)
	);
}

/** A short amount for a chart axis: $0, $500, $1.5K, $12K. */
export function compact(minor: number, currency: string): string {
	const format = new Intl.NumberFormat(tag(), {
		style: 'currency',
		currency,
		currencyDisplay: 'narrowSymbol',
		notation: 'compact'
	});
	const digits =
		new Intl.NumberFormat('en', { style: 'currency', currency }).resolvedOptions()
			.maximumFractionDigits ?? 2;
	return format.format(minor / 10 ** digits);
}

/** A YYYY-MM month in short, for an axis: "may", "May". */
export function monthShort(yearMonth: string): string {
	return new Intl.DateTimeFormat(tag(), { month: 'short', timeZone: 'UTC' })
		.format(utc(`${yearMonth}-01`))
		.replace('.', '');
}

/** The « · » between the parts of a line. The dot holds to the part after it, so a line that wraps starts with it
 * instead of ending on it (pass 3). */
export const LIST = ' ·\u00a0';

/** Items joined the way the language joins them: "a, b and c", "a, b y c". */
export function list(items: string[]): string {
	return new Intl.ListFormat(tag(), { type: 'conjunction' }).format(items);
}

/** A date range, both ends included. Each date holds together (no-break spaces); a line may break only at the dash. */
export function span(from: string, to: string): string {
	const whole = (iso: string) => day(iso).replaceAll(' ', '\u00a0');
	return from === to ? whole(from) : `${whole(from)} – ${whole(to)}`;
}

/** Today in the browser's time zone, as YYYY-MM-DD. */
export function today(): string {
	const now = new Date();
	const pad = (n: number) => String(n).padStart(2, '0');
	return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}
