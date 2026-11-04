// How accounts are put into words.
import type { Schemas } from '#lib/api/client.ts';
import { has, t, type Key } from '#lib/i18n.svelte.ts';

// A new kind in the contract fails the type check until it has a name here.
const kindKeys = {
	checking: 'kind.checking',
	savings: 'kind.savings',
	credit_card: 'kind.credit_card',
	cash: 'kind.cash',
	loan: 'kind.loan',
	investment: 'kind.investment'
} satisfies Record<Schemas['AccountKind'], Key>;

export const kinds = Object.keys(kindKeys) as Schemas['AccountKind'][];
export const kindName = (kind: Schemas['AccountKind']) => t(kindKeys[kind]);

/** A bank format's name in the page's language; the profile's own (English) name when there is no text for it. */
export const formatName = (profile: { id: string; name: string }) => {
	const key = `profile.${profile.id}`;
	return has(key) ? t(key) : profile.name;
};
