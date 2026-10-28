// The import flow through the browser, against a real server (see run.sh): log in, create an account, preview
// and import a statement, import it again, and read the coverage.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type BrowserContext, type Locator, type Page } from '@playwright/test';
import { totp } from './totp.ts';

const fixture = (name: string) =>
	fileURLToPath(new URL(`../../spec/fixtures/bac-account/${name}`, import.meta.url));
const statement = {
	name: 'input.csv',
	mimeType: 'text/csv',
	buffer: readFileSync(fixture('input.csv'))
};
const expected = JSON.parse(readFileSync(fixture('expected.json'), 'utf8'));
const transactions: number = expected.result.transactions.length;
const unreadable: { row: number }[] = expected.result.errors;

const email = process.env.E2E_EMAIL ?? '';
const password = process.env.E2E_PASSWORD ?? '';
const secret = process.env.E2E_SECRET ?? '';

// A TOTP code is accepted once, so the run logs in once and the later tests reuse that session.
test.describe.configure({ mode: 'serial' });
let session: Awaited<ReturnType<BrowserContext['storageState']>>;

async function loggedIn(context: BrowserContext): Promise<Page> {
	await context.addCookies(session.cookies);
	return context.newPage();
}

async function overflow(page: Page): Promise<number> {
	return page.evaluate(
		() => document.documentElement.scrollWidth - document.documentElement.clientWidth
	);
}

function count(card: Locator, label: string): Locator {
	return card
		.locator('.counts div', { has: card.page().getByText(label, { exact: true }) })
		.locator('dd');
}

test('logs in with the password and a current code', async ({ page, context }) => {
	expect(secret, 'run through e2e/run.sh').not.toBe('');
	await page.goto('/import');
	await expect(page).toHaveURL(/\/login\?next=%2Fimport$/);

	await page.getByLabel('Email').fill(email);
	await page.getByLabel('Password').fill(password);
	const right = totp(secret);
	const wrong = String((Number(right) + 500_000) % 1_000_000).padStart(6, '0');
	await page.getByLabel('Code').fill(wrong);
	await page.getByRole('button', { name: 'Log in' }).click();
	await expect(page.getByRole('alert')).toContainText('not right');

	await page.getByLabel('Code').fill(right);
	await page.getByRole('button', { name: 'Log in' }).click();
	await expect(page).toHaveURL(/\/import$/);
	await expect(page.getByRole('heading', { name: 'Import bank files' })).toBeVisible();

	// The session cookie (Secure, HttpOnly) survives a reload over http://127.0.0.1.
	await page.reload();
	await expect(page.getByRole('heading', { name: 'Import bank files' })).toBeVisible();
	session = await context.storageState();
});

test('previews, imports, and re-importing adds nothing', async ({ context }) => {
	const page = await loggedIn(context);
	await page.goto('/accounts');
	await page.getByLabel('Name').fill('Checking');
	await page.getByLabel('Bank file').selectOption('bac-account');
	await page.getByLabel('Key').fill('bac');
	await page.getByRole('button', { name: 'Create account' }).click();
	await expect(page.getByRole('status')).toHaveText('Created Checking.');

	await page.getByRole('link', { name: 'Import' }).click();
	await page
		.getByLabel('Choose files')
		.setInputFiles([
			statement,
			{ name: 'notes.txt', mimeType: 'text/plain', buffer: Buffer.from('just, some\nwords') }
		]);

	const card = page.getByRole('article', { name: 'input.csv' });
	await expect(card.getByLabel('Account').locator('option:checked')).toHaveText('Checking');
	await expect(card.getByText('Found from the file.')).toBeVisible();
	await expect(count(card, 'New')).toHaveText(String(transactions));
	await expect(count(card, 'Already imported')).toHaveText('0');
	await expect(count(card, 'Unreadable')).toHaveText(String(unreadable.length));
	for (const failure of unreadable) {
		await expect(card.getByText(`Row ${failure.row}:`)).toBeVisible();
	}

	// A file no account recognizes asks for the account, and the server says why it cannot read it.
	const other = page.getByRole('article', { name: 'notes.txt' });
	await expect(other.getByText('No account recognizes this file: choose one.')).toBeVisible();
	await other.getByLabel('Account').selectOption({ label: 'Checking' });
	await expect(other.getByRole('alert')).not.toBeEmpty();

	await card.getByRole('button', { name: `Import ${transactions} new` }).click();
	await expect(card.getByRole('status')).toHaveText(
		`Added ${transactions}; skipped 0 already imported.`
	);
	await expect(count(card, 'New')).toHaveText('0');

	// The same file again: everything is already there, and the import button stays off.
	await page.getByLabel('Choose files').setInputFiles(statement);
	const again = page.getByRole('article', { name: 'input.csv' }).nth(1);
	await expect(count(again, 'New')).toHaveText('0');
	await expect(count(again, 'Already imported')).toHaveText(String(transactions));
	await expect(again.getByRole('status')).toHaveText(
		'Every row is already imported: there is nothing to add.'
	);
	await expect(again.getByRole('button', { name: 'Import 0 new' })).toBeDisabled();

	const listed = await page.request.get('/api/v1/transactions?limit=1000');
	expect((await listed.json()).items).toHaveLength(transactions);
});

test('a manual category survives applying the rules', async ({ context }) => {
	const page = await loggedIn(context);
	await page.goto('/rules');
	for (const name of ['Groceries', 'Other']) {
		await page.getByLabel('Category name').fill(name);
		await page.getByRole('button', { name: 'Create category' }).click();
		await expect(page.getByRole('status')).toHaveText(`Created ${name}.`);
	}
	await page.getByLabel('Rule name').fill('Market');
	await page.getByLabel('Description has').fill('SUPERMERCADO');
	await page.getByLabel('Money').selectOption('out');
	await page.getByLabel('Sets category').selectOption({ label: 'Groceries' });
	await page.getByRole('button', { name: 'Add rule' }).click();
	await expect(page.getByRole('listitem').filter({ hasText: 'Market' })).toContainText(
		'text has «SUPERMERCADO» · money out'
	);

	// Imported before the rule existed, so nothing is sorted yet: choose one by hand.
	await page.getByRole('link', { name: 'Uncategorized' }).click();
	await expect(page.getByText(`${transactions} without a category.`)).toBeVisible();
	const market = page.getByRole('listitem').filter({ hasText: 'SUPERMERCADO' });
	await expect(market).toHaveCount(2);
	await market.first().getByLabel('Category').selectOption({ label: 'Other' });
	await expect(page.getByText(`${transactions - 1} without a category.`)).toBeVisible();

	await page.getByRole('link', { name: 'Rules' }).click();
	await page.getByRole('button', { name: 'Check changes' }).click();
	await expect(page.getByText('1 transaction would change:')).toBeVisible();
	await expect(page.getByText('Category: — → Groceries')).toBeVisible();
	await page.getByRole('button', { name: 'Apply 1 change' }).click();
	await expect(page.getByRole('status').filter({ hasText: 'Applied' })).toHaveText(
		'Applied 1 change.'
	);
	await expect(page.getByText('Nothing would change.')).toBeVisible();

	const listed = await page.request.get('/api/v1/transactions?limit=1000');
	const sorted = (await listed.json()).items.filter((t: { description: string }) =>
		t.description.includes('SUPERMERCADO')
	);
	const by = sorted.map((t: { category_set_by: string }) => t.category_set_by).sort();
	expect(by).toEqual(['manual', 'rule']);
});

test('coverage shows what was imported', async ({ context }) => {
	const page = await loggedIn(context);
	await page.goto('/coverage');
	const row = page.getByRole('row', { name: /Checking/ });
	await expect(row).toContainText('2026-05-31');
	await expect(row).toContainText('2026-05-02 – 2026-05-31');
	// Months behind any real today: shown as late, in words.
	await expect(row.locator('.chip')).toHaveText('Late');
});

test('every page fits a 390 px screen', async ({ browser }) => {
	const phone = await browser.newContext({ viewport: { width: 390, height: 844 } });
	const pages: [string, string][] = [
		['/login', 'Coinroost'],
		['/import', 'Import bank files'],
		['/coverage', 'Coverage'],
		['/accounts', 'Accounts'],
		['/rules', 'Rules'],
		['/uncategorized', 'Uncategorized']
	];
	for (const [path, heading] of pages) {
		if (path !== '/login') await phone.addCookies(session.cookies);
		const page = await phone.newPage();
		await page.goto(path);
		await expect(page.getByRole('heading', { name: heading, exact: true })).toBeVisible();
		expect(await overflow(page), path).toBeLessThanOrEqual(0);
		if (path === '/import') {
			// With a card open and its transactions table expanded (the account made in the import test reads
			// it), the table scrolls in its own box.
			await page.getByLabel('Choose files').setInputFiles(statement);
			const card = page.getByRole('article', { name: 'input.csv' });
			await card.getByText(/^Show the \d+ transactions$/).click();
			await expect(card.getByRole('table')).toBeVisible();
			expect(await overflow(page), 'import with a table open').toBeLessThanOrEqual(0);
		}
		await page.close();
	}
	await phone.close();
});
