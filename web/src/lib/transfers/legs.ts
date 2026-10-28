// How transfer legs read on screen.
import type { Schemas } from '#lib/api/client.ts';
import { money } from '#lib/format.ts';

export type AccountNames = Map<string, string>;

export function accountNames(accounts: Schemas['Account'][]): AccountNames {
	return new Map(accounts.map((a) => [a.id, a.name]));
}

export function accountName(id: string, names: AccountNames): string {
	return names.get(id) ?? 'an account';
}

/** "2026-05-15 · Checking · -$500.00 · TRANSFER TO SAVINGS" */
export function legText(leg: Schemas['TransferLeg'], names: AccountNames): string {
	return [
		leg.date,
		accountName(leg.account_id, names),
		money(leg.amount_minor, leg.currency),
		leg.description
	].join(' · ');
}

export function passLabel(pass: number): string {
	switch (pass) {
		case 1:
			return 'the only match within two days';
		case 2:
			return 'the text names the account';
		default:
			return 'sent on a Friday, arrived on Monday';
	}
}

export function reasonLabel(reason: Schemas['Reason']): string {
	return reason === 'no_counterpart'
		? 'No other side within two days: its statement may not be imported yet.'
		: 'Several legs could be its other side: choose one.';
}
