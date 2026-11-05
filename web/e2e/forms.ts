// What the forms keep closed, opened as a person opens it.
import { expect, type Page } from '@playwright/test';

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
