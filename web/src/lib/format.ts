// How amounts and dates are shown.

/** Minor units as an amount in the currency, such as -4599 USD → -$45.99. */
export function money(minor: number, currency: string): string {
	const format = new Intl.NumberFormat('en-US', { style: 'currency', currency });
	const digits = format.resolvedOptions().maximumFractionDigits ?? 2;
	return format.format(minor / 10 ** digits);
}

/** A date range, both ends included. */
export function span(from: string, to: string): string {
	return from === to ? from : `${from} – ${to}`;
}

/** Today in the browser's time zone, as YYYY-MM-DD. */
export function today(): string {
	const now = new Date();
	const pad = (n: number) => String(n).padStart(2, '0');
	return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}
