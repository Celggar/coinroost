// How transfer legs read on screen.
import type { Schemas } from '#lib/api/client.ts';
import { day, money } from '#lib/format.ts';
import { t } from '#lib/i18n.svelte.ts';

export type AccountNames = Map<string, string>;

export function accountNames(accounts: Schemas['Account'][]): AccountNames {
	return new Map(accounts.map((a) => [a.id, a.name]));
}

export function accountName(id: string, names: AccountNames): string {
	return names.get(id) ?? t('legs.an_account');
}

/** "May 15, 2026 · Checking · -$500.00 · TRANSFER TO SAVINGS" */
export function legText(leg: Schemas['TransferLeg'], names: AccountNames): string {
	return [
		day(leg.date),
		accountName(leg.account_id, names),
		money(leg.amount_minor, leg.currency),
		leg.description
	].join(' · ');
}

export function passLabel(pass: number): string {
	switch (pass) {
		case 1:
			return t('legs.pass_1');
		case 2:
			return t('legs.pass_2');
		default:
			return t('legs.pass_3');
	}
}

export function reasonLabel(reason: Schemas['Reason']): string {
	return reason === 'no_counterpart' ? t('legs.no_counterpart') : t('legs.ambiguous');
}
