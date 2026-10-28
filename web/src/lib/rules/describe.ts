// How rules, and what they change, are put into words.
import type { Schemas } from '#lib/api/client.ts';

export type Names = {
	categories: Map<string, string>;
	payees: Map<string, string>;
	accounts: Map<string, string>;
};

export function names(
	categories: Schemas['Category'][],
	payees: Schemas['Payee'][],
	accounts: Schemas['Account'][]
): Names {
	return {
		categories: new Map(categories.map((c) => [c.id, c.name])),
		payees: new Map(payees.map((p) => [p.id, p.name])),
		accounts: new Map(accounts.map((a) => [a.id, a.name]))
	};
}

/** Minor units as a plain amount with two decimals. */
export function plain(minor: number): string {
	return (minor / 100).toFixed(2);
}

/** A typed amount in minor units; `null` when empty, `NaN` when it is not a number. */
export function toMinor(text: string): number | null {
	const trimmed = text.trim();
	if (trimmed === '') return null;
	if (!/^\d+(\.\d{1,2})?$/.test(trimmed)) return Number.NaN;
	return Math.round(Number(trimmed) * 100);
}

export function conditions(rule: Schemas['Rule'], names: Names): string[] {
	const when = rule.when;
	const parts: string[] = [];
	if (when.description_contains != null) parts.push(`text has «${when.description_contains}»`);
	if (when.direction === 'out') parts.push('money out');
	if (when.direction === 'in') parts.push('money in');
	if (when.account_id) parts.push(`account ${names.accounts.get(when.account_id) ?? '?'}`);
	if (when.amount_min != null && when.amount_max != null) {
		parts.push(`amount ${plain(when.amount_min)} to ${plain(when.amount_max)}`);
	} else if (when.amount_min != null) {
		parts.push(`amount ${plain(when.amount_min)} or more`);
	} else if (when.amount_max != null) {
		parts.push(`amount up to ${plain(when.amount_max)}`);
	}
	return parts;
}

export function actions(rule: Schemas['Rule'], names: Names): string[] {
	const then = rule.then;
	const parts: string[] = [];
	if (then.category_id) parts.push(`category ${names.categories.get(then.category_id) ?? '?'}`);
	if (then.payee_id) parts.push(`payee ${names.payees.get(then.payee_id) ?? '?'}`);
	if (then.payee_from_text) parts.push('payee: the name after the text');
	return parts;
}

export function categoryName(id: string | null | undefined, names: Names): string {
	return id ? (names.categories.get(id) ?? '?') : '—';
}

export function payeeName(sorting: Schemas['Sorting'], names: Names): string {
	if (sorting.new_payee) return `${sorting.new_payee} (new)`;
	return sorting.payee_id ? (names.payees.get(sorting.payee_id) ?? '?') : '—';
}
