// What the forms keep closed, opened as a person opens it.
import { expect, type Page } from '@playwright/test';
import { say } from './messages.ts';

/** Opens the fold of the form on the page: «Opciones avanzadas», «Más condiciones». */
export const unfold = (page: Page) => page.locator('form details:not([open]) summary').click();

/** One-legged transactions in a transfer category, by account name: legs waiting for their other side. */
export async function postLegs(
	page: Page,
	category: string,
	description: string,
	rows: [account: string, amount_minor: number, date: string][]
) {
	const items = async (url: string): Promise<{ id: string; name: string }[]> =>
		(await (await page.request.get(url)).json()).items;
	const accounts = await items('/api/v1/accounts');
	const own = (await items('/api/v1/categories')).find((c) => c.name === category);
	for (const [name, amount_minor, date] of rows) {
		const account_id = accounts.find((a) => a.name === name)?.id;
		const legs = [{ account_id, amount_minor, statement_text: description }];
		const data = { date, description, category_id: own?.id, legs };
		expect((await page.request.post('/api/v1/transactions', { data })).status()).toBe(201);
	}
}

/** The pairing by hand offers only what cancels a leg, and says one reason once. */
export async function checkByHand(page: Page) {
	await postLegs(page, 'Propias', 'MANO', [
		['Corriente', -70000, '2026-03-02'],
		['Ahorro', 70000, '2026-03-20'],
		['Ahorro', 90000, '2026-03-02']
	]);
	await page.goto('/transfers');
	await expect(page.getByText(say('legs.no_counterpart'))).toHaveCount(1);
	const form = page.getByRole('form', { name: say('unmatched.by_hand') });
	const one = form.getByLabel(say('unmatched.one_side'));
	const other = form.getByLabel(say('unmatched.other_side'));
	const odd = '2 mar 2026 · Ahorro · +$900.00 · MANO';
	await other.selectOption({ label: odd });
	await one.selectOption({ label: '2 mar 2026 · Corriente · −$700.00 · MANO' });
	await expect(other).toHaveValue('');
	await expect(other.getByRole('option', { name: odd })).toBeDisabled();
	await expect(
		other.getByRole('option', { name: '20 mar 2026 · Ahorro · +$700.00' })
	).toBeEnabled();
	await one.selectOption({ label: odd });
	await expect(form.getByText(say('unmatched.nothing_fits'))).toBeVisible();
}
