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
	return format.format(minor / 10 ** digits);
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

/** Items joined the way the language joins them: "a, b and c", "a, b y c". */
export function list(items: string[]): string {
	return new Intl.ListFormat(tag(), { type: 'conjunction' }).format(items);
}

/** A date range, both ends included. */
export function span(from: string, to: string): string {
	return from === to ? day(from) : `${day(from)} – ${day(to)}`;
}

/** Today in the browser's time zone, as YYYY-MM-DD. */
export function today(): string {
	const now = new Date();
	const pad = (n: number) => String(n).padStart(2, '0');
	return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}
