// The queue's user and rows, for its specs: its own user and its own data, made through the API.
import { expect, type Browser, type BrowserContext, type Page } from '@playwright/test';
import { say } from './messages.ts';
import { totp } from './totp.ts';

// A TOTP code is accepted once, so the run logs in once and the later tests reuse that session.
export let session: Awaited<ReturnType<BrowserContext['storageState']>>;
let account = '';
export let food = '';

async function post(page: Page, path: string, data: unknown) {
	const response = await page.request.post(path, { data });
	expect(response.ok(), path).toBe(true);
	return response.json();
}

export const spend = (description: string, amount_minor: number) => ({
	date: '2026-05-10',
	description,
	legs: [{ account_id: account, amount_minor, statement_text: description }]
});

export async function logIn(browser: Browser) {
	const secret = process.env.E2E_CATEGORIZE_SECRET ?? '';
	expect(secret, 'run through e2e/run.sh').not.toBe('');
	const page = await browser.newPage({ locale: 'es-PA' });
	await page.goto('/uncategorized');
	await page.getByLabel(say('login.email')).fill(process.env.E2E_CATEGORIZE_EMAIL ?? '');
	await page.getByLabel(say('login.password')).fill(process.env.E2E_CATEGORIZE_PASSWORD ?? '');
	await page.getByLabel(say('login.code')).fill(totp(secret));
	await page.getByRole('button', { name: say('login.title') }).click();
	await expect(page.getByRole('heading', { name: say('uncategorized.title') })).toBeVisible();
	session = await page.context().storageState();
	account = (
		await post(page, '/api/v1/accounts', {
			name: 'Cash',
			kind: 'cash',
			currency: 'USD',
			namespace: 'cash-queue'
		})
	).id;
	food = (await post(page, '/api/v1/categories', { name: 'Food', kind: 'expense' })).id;
	await post(page, '/api/v1/categories', { name: 'Pay', kind: 'income' });
	for (const line of [
		spend('CAFE CENTRAL 0042', -375),
		spend('CAFE CENTRAL 0099', -420),
		spend('MARKET ONE', -11326),
		spend('SALARY MAY', 146216)
	])
		await post(page, '/api/v1/transactions', line);
	await page.close();
}

export async function queue(context: BrowserContext): Promise<Page> {
	await context.addCookies(session.cookies);
	const page = await context.newPage();
	await page.goto('/uncategorized');
	await expect(page.getByRole('list', { name: say('uncategorized.list') })).toBeVisible();
	return page;
}

export const rows = (page: Page) =>
	page.getByRole('list', { name: say('uncategorized.list') }).getByRole('listitem');
export const row = (page: Page, text: string) => rows(page).filter({ hasText: text });
export const choose = (page: Page, text: string, category: string) =>
	row(page, text).getByLabel(say('column.category')).selectOption({ label: category });
export const filter = (page: Page, name: 'all' | 'spending' | 'income') =>
	page.getByRole('radio', { name: new RegExp(say(`uncategorized.${name}`)) });

/** Rows of its own: the queue is empty between tests. */
export async function fill(context: BrowserContext, lines: ReturnType<typeof spend>[]) {
	await context.addCookies(session.cookies);
	for (const line of lines) {
		const made = await context.request.post('/api/v1/transactions', { data: line });
		expect(made.ok()).toBe(true);
	}
}

/** Sorts what a test left waiting, so the next finds the queue empty. */
export async function sortAll(page: Page) {
	const list = await page.request.get('/api/v1/transactions?uncategorized=true&limit=1000');
	for (const { id } of (await list.json()).items) {
		const sorted = await page.request.patch(`/api/v1/transactions/${id}`, {
			data: { category_id: food }
		});
		expect(sorted.ok()).toBe(true);
	}
}
