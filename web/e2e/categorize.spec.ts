// The queue of transactions with no category, through the browser, against a real server (see run.sh): rows that
// filter and sort, a choice that stays with its Deshacer until the next one, and a choice turned into a rule on the
// spot. Its own user and its own data, made through the API.
import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import { say } from './messages.ts';
import { totp } from './totp.ts';

// A TOTP code is accepted once, so the run logs in once and the later tests reuse that session.
test.describe.configure({ mode: 'serial' });
test.use({ locale: 'es-PA' });
let session: Awaited<ReturnType<BrowserContext['storageState']>>;
let account = '';
let food = '';

async function post(page: Page, path: string, data: unknown) {
	const response = await page.request.post(path, { data });
	expect(response.ok(), path).toBe(true);
	return response.json();
}

const spend = (description: string, amount_minor: number) => ({
	date: '2026-05-10',
	description,
	legs: [{ account_id: account, amount_minor, statement_text: description }]
});

test.beforeAll(async ({ browser }) => {
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
});

async function queue(context: BrowserContext): Promise<Page> {
	await context.addCookies(session.cookies);
	const page = await context.newPage();
	await page.goto('/uncategorized');
	await expect(page.getByRole('list', { name: say('uncategorized.list') })).toBeVisible();
	return page;
}

const rows = (page: Page) =>
	page.getByRole('list', { name: say('uncategorized.list') }).getByRole('listitem');
const row = (page: Page, text: string) => rows(page).filter({ hasText: text });
const choose = (page: Page, text: string, category: string) =>
	row(page, text).getByLabel(say('column.category')).selectOption({ label: category });

test('rows filter by kind and sort by amount, each a line of about 52 px', async ({ context }) => {
	const page = await queue(context);
	await page.setViewportSize({ width: 1280, height: 800 });
	await expect(rows(page)).toHaveCount(4);
	const height = await rows(page)
		.first()
		.evaluate((e) => e.getBoundingClientRect().height);
	expect(height).toBeLessThanOrEqual(56);

	await page.getByRole('radio', { name: new RegExp(say('uncategorized.income')) }).check();
	await expect(rows(page)).toHaveCount(1);
	await expect(rows(page)).toContainText('SALARY MAY');
	await page.getByRole('radio', { name: new RegExp(say('uncategorized.spending')) }).check();
	await expect(rows(page)).toHaveCount(3);

	await page.getByRole('button', { name: say('uncategorized.by_amount') }).click();
	await expect(rows(page).first()).toContainText('MARKET ONE');
});

test('a choice stays with its Deshacer until the next one, and the focus goes on', async ({
	context
}) => {
	const page = await queue(context);
	await choose(page, 'MARKET ONE', 'Food');
	// The row stays and says what it took; the next choice is one key away.
	await expect(row(page, 'MARKET ONE')).toContainText('Food');
	const undo = row(page, 'MARKET ONE').getByRole('button', { name: say('uncategorized.undo') });
	await expect(undo).toBeVisible();
	await expect(row(page, 'SALARY MAY').getByLabel(say('column.category'))).toBeFocused();

	// Deshacer: the server forgets it, the select comes back and takes the focus.
	const cleared = page.waitForResponse(
		(r) => r.request().method() === 'PATCH' && r.request().postData() === '{"category_id":null}'
	);
	await undo.click();
	expect((await cleared).ok()).toBe(true);
	const select = row(page, 'MARKET ONE').getByLabel(say('column.category'));
	await expect(select).toBeFocused();
	await expect(select).toHaveValue('');

	// A choice the server refuses leaves the row waiting, and says so.
	await page.route('**/api/v1/transactions/*', (route) =>
		route.request().method() === 'PATCH' ? route.fulfill({ status: 500 }) : route.fallback()
	);
	await choose(page, 'MARKET ONE', 'Food');
	await expect(page.getByRole('alert').first()).toHaveText(say('uncategorized.set_failed'));
	await expect(select).toHaveValue('');
	await page.unroute('**/api/v1/transactions/*');

	// The next choice is what lets the last row go.
	await choose(page, 'MARKET ONE', 'Food');
	await expect(row(page, 'MARKET ONE').getByRole('button')).toHaveCount(3);
	await choose(page, 'SALARY MAY', 'Pay');
	await expect(row(page, 'MARKET ONE')).toHaveCount(0);
	await expect(rows(page)).toHaveCount(3);
	await page.close();
});

test('a choice becomes a rule, and the rule sorts its twin', async ({ context }) => {
	const page = await queue(context);
	await choose(page, 'CAFE CENTRAL 0042', 'Food');
	const field = row(page, 'CAFE CENTRAL 0042').getByLabel(say('uncategorized.offer'));
	// Prefilled without what ends the description with a digit, and editable.
	await expect(field).toHaveValue('CAFE CENTRAL');
	await field.fill('CAFE CENTR');
	await row(page, 'CAFE CENTRAL 0042')
		.getByRole('button', { name: say('uncategorized.make_rule') })
		.click();
	// Its change to what is already here, shown where the choice was made.
	await expect(page.locator('#changes-some')).toBeFocused();
	await expect(page.locator('.changes').getByRole('listitem')).toContainText('CAFE CENTRAL 0099');
	await page.getByRole('button', { name: new RegExp(say('changes.apply_one')) }).click();
	await expect(row(page, 'CAFE CENTRAL')).toHaveCount(0);

	const rules = await (await page.request.get('/api/v1/rules')).json();
	const rule = rules.items.find((r: { name: string }) => r.name === 'CAFE CENTR');
	expect(rule.when.description_contains).toBe('CAFE CENTR');
	expect(rule.then.category_id).toBe(food);
	await page.close();
});

test('a sorted row fades before the next moves up, and a count never shows twice', async ({
	context
}) => {
	const page = await queue(context);
	await page.setViewportSize({ width: 1280, height: 720 });
	for (const text of ['COFFEE SHOP', 'BOOK STORE', 'HARDWARE STORE', 'BAKERY'])
		await post(page, '/api/v1/transactions', spend(text, -1000));
	type Frames = { frames: [number, number][] };
	for (const reducedMotion of ['no-preference', 'reduce'] as const) {
		await page.emulateMedia({ reducedMotion });
		await page.goto('/uncategorized');
		const select = page.locator('.queue select').first();
		await expect(select).toBeEnabled();
		// A sorted row stays until the next choice, which is what makes it leave.
		await select.selectOption({ index: 1 });
		await expect(page.locator('.queue > li.sorted')).toHaveCount(1);
		const next = page.locator('.queue > li:not(.sorted) select').first();
		await expect(next).toBeFocused();
		// Every frame: how many values a count shows at once, and how far the next row overlaps the leaving one while it
		// shows. A count read double as the new one rose under the old; the next card sat under the fading one (pass 16).
		await page.evaluate(() => {
			const w = window as unknown as Frames;
			w.frames = [];
			const leaving = document.querySelector('.queue > li.sorted')!;
			const next = leaving.nextElementSibling!;
			const step = () => {
				const counts = Math.max(
					...[...document.querySelectorAll('.tick')].map(
						(tick) =>
							[...tick.children].filter((e) => Number(getComputedStyle(e).opacity) > 0.02).length
					)
				);
				const a = leaving.getBoundingClientRect();
				const b = next.getBoundingClientRect();
				const shown = leaving.isConnected && Number(getComputedStyle(leaving).opacity) > 0.02;
				w.frames.push([
					counts,
					shown ? Math.max(0, Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top)) : 0
				]);
				if (w.frames.length < 90) requestAnimationFrame(step);
			};
			requestAnimationFrame(step);
		});
		await next.selectOption({ index: 1 });
		await page.waitForFunction(() => (window as unknown as Frames).frames.length >= 90);
		const frames = await page.evaluate(() => (window as unknown as Frames).frames);
		expect(
			frames.filter(([counts]) => counts > 1),
			reducedMotion
		).toEqual([]);
		expect(
			frames.filter(([, overlap]) => overlap > 0),
			reducedMotion
		).toEqual([]);
	}
	await page.emulateMedia({ reducedMotion: null });
});
