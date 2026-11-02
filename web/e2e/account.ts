// The account's controls on a wide screen: a button at the rail's foot opens a panel with the language and
// logging out (P2). The button is named «Cuenta: <email>» (or «Account: …» in English).
import type { Locator, Page } from '@playwright/test';
import { english, say } from './messages.ts';

/** The rail's account button, in the page's language. */
export function accountButton(page: Page, language: 'es' | 'en' = 'es'): Locator {
	const name = (language === 'es' ? say : english)('nav.account');
	return page.getByRole('button', { name: new RegExp(`^${name}: `) });
}

/** Opens the account's panel, if it is not open yet. */
export async function openAccount(page: Page, language: 'es' | 'en' = 'es') {
	const button = accountButton(page, language);
	if ((await button.getAttribute('aria-expanded')) !== 'true') await button.click();
}

/** Picks a language in the account's panel. */
export async function pickLanguage(page: Page, name: string, language: 'es' | 'en' = 'es') {
	await openAccount(page, language);
	await page.getByRole('radio', { name }).check();
}

/** Logs out through the account's panel. */
export async function logOut(page: Page) {
	await openAccount(page);
	await page.getByRole('button', { name: say('nav.log_out'), exact: true }).click();
}
