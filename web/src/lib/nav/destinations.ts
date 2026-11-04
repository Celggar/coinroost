// Where the app's navigation goes (P1). Five destinations in one order and the same words at every width (WAI menu
// structure): Resumen, Importar, Revisar, Cuentas, then Reglas on a wide screen or «Más» on a phone. Revisar and
// Cuentas each hold two pages, shown as tabs; every old URL still loads.
import type { Key } from '#lib/i18n.svelte.ts';

export type IconName =
	| 'home'
	| 'import'
	| 'review'
	| 'bank'
	| 'rules'
	| 'more'
	| 'shield'
	| 'logout'
	| 'chev'
	| 'check'
	| 'unknown'
	| 'tag'
	| 'clock'
	| 'transfer'
	| 'cash';

export type Destination = {
	href: string;
	label: Key;
	icon: IconName;
	/** The pages where this destination is the current one. */
	pages: string[];
	/** Which count its badge shows, if any. */
	badge?: 'review' | 'accounts';
};

const summary: Destination = { href: '/', label: 'nav.summary', icon: 'home', pages: ['/'] };
const imports: Destination = {
	href: '/import',
	label: 'nav.import',
	icon: 'import',
	pages: ['/import']
};
const review: Destination = {
	href: '/uncategorized',
	label: 'nav.review',
	icon: 'review',
	pages: ['/uncategorized', '/transfers'],
	badge: 'review'
};
const accounts: Destination = {
	href: '/accounts',
	label: 'nav.accounts',
	icon: 'bank',
	pages: ['/accounts', '/coverage'],
	badge: 'accounts'
};
const rules: Destination = {
	href: '/rules',
	label: 'nav.rules',
	icon: 'rules',
	pages: ['/rules', '/categories']
};
const more: Destination = {
	href: '/more',
	label: 'nav.more',
	icon: 'more',
	pages: ['/more', '/rules', '/categories', '/security']
};

/** The rail on a wide screen. */
export const wide = [summary, imports, review, accounts, rules];
/** The bar on a phone. */
export const narrow = [summary, imports, review, accounts, more];

/** The two pages under Revisar, Cuentas and Reglas, shown as tabs at the top of each. */
export type Tab = {
	href: string;
	label: Key;
	/** Which count its badge shows, so a destination's badge says which of its pages holds it. */
	badge?: 'uncategorized' | 'transfers' | 'accounts';
};

export const groups: { label: Key; tabs: Tab[] }[] = [
	{
		label: 'nav.review',
		tabs: [
			{ href: '/uncategorized', label: 'nav.uncategorized', badge: 'uncategorized' },
			{ href: '/transfers', label: 'nav.transfers', badge: 'transfers' }
		]
	},
	{
		label: 'nav.accounts',
		tabs: [
			{ href: '/accounts', label: 'nav.accounts' },
			{ href: '/coverage', label: 'nav.coverage', badge: 'accounts' }
		]
	},
	{
		label: 'nav.rules',
		tabs: [
			{ href: '/rules', label: 'nav.rules' },
			{ href: '/categories', label: 'categories.title' }
		]
	}
];

/** The page's title for the phone's top bar: its destination's name. */
export function titleOf(path: string): Key {
	if (path === '/security') return 'nav.security';
	if (path === '/rules' || path === '/categories') return 'nav.rules';
	return [...narrow].find((d) => d.pages.includes(path))?.label ?? 'app.name';
}

/** The error page's title, for its heading and the phone's top bar. */
export function errorTitle(status: number): Key {
	return status === 404 ? 'error.not_found_title' : 'error.failed_title';
}

export function isCurrent(destination: Destination, path: string): boolean {
	return destination.pages.includes(path);
}
