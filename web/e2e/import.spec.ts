// The app through the browser in Spanish, against a real server (see run.sh): log in, create an account, preview
// and import a statement, import it again, sort, pair transfers, read the coverage and the dashboard, and switch
// to English.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type BrowserContext, type Locator, type Page } from '@playwright/test';
import { checkDashboard, checkPayroll, loadEmployers } from './dashboard.ts';
import { english, say } from './messages.ts';
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
// Panama's Spanish, as the person's browser asks for it.
test.use({ locale: 'es-PA' });
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

	await page.getByLabel(say('login.email')).fill(email);
	await page.getByLabel(say('login.password')).fill(password);
	const right = totp(secret);
	const wrong = String((Number(right) + 500_000) % 1_000_000).padStart(6, '0');
	await page.getByLabel(say('login.code')).fill(wrong);
	await page.getByRole('button', { name: say('login.title') }).click();
	await expect(page.getByRole('alert')).toHaveText(say('login.wrong'));

	await page.getByLabel(say('login.code')).fill(right);
	await page.getByRole('button', { name: say('login.title') }).click();
	await expect(page).toHaveURL(/\/import$/);
	await expect(page.getByRole('heading', { name: say('import.title') })).toBeVisible();

	// The session cookie (Secure, HttpOnly) survives a reload over http://127.0.0.1.
	await page.reload();
	await expect(page.getByRole('heading', { name: say('import.title') })).toBeVisible();
	session = await context.storageState();
});

test('previews, imports, and re-importing adds nothing', async ({ context }) => {
	const page = await loggedIn(context);
	await page.goto('/accounts');
	await page.getByLabel(say('accounts.name')).fill('Checking');
	await page.getByLabel(say('accounts.bank_file')).selectOption('bac-account');
	await page.getByLabel(say('accounts.key')).fill('bac');
	await page.getByRole('button', { name: say('accounts.create') }).click();
	await expect(page.getByRole('status')).toHaveText(say('accounts.created', { name: 'Checking' }));

	await page.getByRole('link', { name: say('nav.import'), exact: true }).click();
	await page
		.getByLabel(say('import.choose'))
		.setInputFiles([
			statement,
			{ name: 'notes.txt', mimeType: 'text/plain', buffer: Buffer.from('just, some\nwords') }
		]);

	const card = page.getByRole('article', { name: 'input.csv' });
	await expect(card.getByLabel(say('file.account')).locator('option:checked')).toHaveText(
		'Checking'
	);
	await expect(card.getByText(say('file.found'))).toBeVisible();
	await expect(count(card, say('file.new'))).toHaveText(String(transactions));
	await expect(count(card, say('file.already'))).toHaveText('0');
	await expect(count(card, say('file.unreadable_rows'))).toHaveText(String(unreadable.length));
	for (const failure of unreadable) {
		const row = say('file.bad_row', { row: failure.row, reason: '' }).trim();
		await expect(card.getByText(row)).toBeVisible();
	}

	// A file no account recognizes asks for the account, and the server says why it cannot read it.
	const other = page.getByRole('article', { name: 'notes.txt' });
	await expect(other.getByText(say('file.not_found'))).toBeVisible();
	await other.getByLabel(say('file.account')).selectOption({ label: 'Checking' });
	await expect(other.getByRole('alert')).not.toBeEmpty();

	await card
		.getByRole('button', { name: say('file.import_other', { count: transactions }) })
		.click();
	await expect(card.getByRole('status')).toHaveText(
		say('file.added', { added: transactions, duplicates: 0 })
	);
	await expect(count(card, say('file.new'))).toHaveText('0');

	// The same file again: everything is already there, and the import button stays off.
	await page.getByLabel(say('import.choose')).setInputFiles(statement);
	const again = page.getByRole('article', { name: 'input.csv' }).nth(1);
	await expect(count(again, say('file.new'))).toHaveText('0');
	await expect(count(again, say('file.already'))).toHaveText(String(transactions));
	await expect(again.getByRole('status')).toHaveText(say('file.nothing_new'));
	await expect(
		again.getByRole('button', { name: say('file.import_other', { count: 0 }) })
	).toBeDisabled();

	const listed = await page.request.get('/api/v1/transactions?limit=1000');
	expect((await listed.json()).items).toHaveLength(transactions);
});

test('a manual category survives applying the rules', async ({ context }) => {
	const page = await loggedIn(context);
	await page.goto('/rules');
	for (const name of ['Groceries', 'Other']) {
		await page.getByLabel(say('categoryform.name')).fill(name);
		await page.getByRole('button', { name: say('categoryform.create') }).click();
		await expect(page.getByRole('status')).toHaveText(say('categoryform.created', { name }));
	}
	await page.getByLabel(say('ruleform.name')).fill('Market');
	await page.getByLabel(say('ruleform.text')).fill('SUPERMERCADO');
	await page.getByLabel(say('ruleform.money')).selectOption('out');
	await page.getByLabel(say('ruleform.category')).selectOption({ label: 'Groceries' });
	await page.getByRole('button', { name: say('ruleform.add') }).click();
	await expect(page.getByRole('listitem').filter({ hasText: 'Market' })).toContainText(
		`${say('when.text', { text: 'SUPERMERCADO' })} · ${say('when.out')}`
	);

	// Imported before the rule existed, so nothing is sorted yet: choose one by hand.
	await page.getByRole('link', { name: say('nav.uncategorized'), exact: true }).click();
	await expect(
		page.getByText(say('uncategorized.count_other', { count: transactions }))
	).toBeVisible();
	const market = page.getByRole('listitem').filter({ hasText: 'SUPERMERCADO' });
	await expect(market).toHaveCount(2);
	await market.first().getByLabel(say('column.category')).selectOption({ label: 'Other' });
	await expect(
		page.getByText(say('uncategorized.count_other', { count: transactions - 1 }))
	).toBeVisible();

	await page.getByRole('link', { name: say('nav.rules'), exact: true }).click();
	await page.getByRole('button', { name: say('changes.check') }).click();
	await expect(page.getByText(say('changes.would_one'))).toBeVisible();
	await expect(
		page.getByText(say('changes.category', { from: '—', to: 'Groceries' }))
	).toBeVisible();
	await page.getByRole('button', { name: say('changes.apply_one') }).click();
	const applied = say('changes.applied_one');
	await expect(page.getByRole('status').filter({ hasText: applied })).toHaveText(applied);
	await expect(page.getByText(say('changes.nothing'))).toBeVisible();

	const listed = await page.request.get('/api/v1/transactions?limit=1000');
	const sorted = (await listed.json()).items.filter((t: { description: string }) =>
		t.description.includes('SUPERMERCADO')
	);
	const by = sorted.map((t: { category_set_by: string }) => t.category_set_by).sort();
	expect(by).toEqual(['manual', 'rule']);
});

test('pairs two legs and splits them', async ({ context }) => {
	const page = await loggedIn(context);
	const post = async (url: string, data: object) => {
		const response = await page.request.post(url, { data });
		expect(response.status(), url).toBe(201);
		return response.json();
	};
	// The other side of the statement's "TRANSF A CUENTA PROPIA" (-500.00 on 2026-05-15), in a second account.
	const own = await post('/api/v1/categories', { name: 'Own transfers', kind: 'transfer' });
	const savings = await post('/api/v1/accounts', {
		name: 'Savings',
		kind: 'savings',
		currency: 'USD',
		namespace: 'savings'
	});
	await post('/api/v1/transactions', {
		date: '2026-05-15',
		description: 'FROM CHECKING',
		category_id: own.id,
		legs: [{ account_id: savings.id, amount_minor: 50000, statement_text: 'FROM CHECKING' }]
	});
	// The outflow becomes a transfer by hand.
	await page.goto('/uncategorized');
	await page
		.getByRole('listitem')
		.filter({ hasText: 'TRANSF A CUENTA PROPIA' })
		.getByLabel(say('column.category'))
		.selectOption({ label: 'Own transfers' });
	await expect(page.getByRole('status')).toHaveText(
		say('uncategorized.done', { description: 'TRANSF A CUENTA PROPIA', category: 'Own transfers' })
	);

	await page.getByRole('link', { name: say('nav.transfers'), exact: true }).click();
	const waiting = page.getByRole('list', { name: say('waiting.title') });
	await expect(waiting.getByRole('listitem')).toHaveCount(1);
	// Dates as Panama reads them: day, short month, year.
	await expect(waiting).toContainText('Checking (15 may 2026) → Savings (15 may 2026)');
	await page.getByLabel(say('period.from'), { exact: true }).fill('2026-05-01');
	await page.getByLabel(say('period.to'), { exact: true }).fill('2026-05-31');
	const unpaired = page.getByRole('definition').nth(2);
	await expect(unpaired).toHaveText('$500.00');
	await page.getByRole('button', { name: say('waiting.pair_one') }).click();
	const paired = say('waiting.paired_one');
	await expect(page.getByRole('status').filter({ hasText: paired })).toHaveText(paired);
	const made = page.getByRole('list', { name: say('made.title') });
	await expect(made.getByRole('listitem')).toHaveCount(1);
	await expect(unpaired).toHaveText('$0.00');

	// Split: the two transactions come back, and the pair waits again.
	await page
		.getByRole('button', {
			name: say('made.split_named', { description: 'TRANSF A CUENTA PROPIA' })
		})
		.click();
	await page.getByRole('button', { name: say('made.split_it') }).click();
	await expect(page.getByText(say('made.none'))).toBeVisible();
	await expect(waiting.getByRole('listitem')).toHaveCount(1);

	// By hand: the same leg twice is refused with the API's reason, which a new choice clears; the right two pair.
	const form = page.getByRole('form', { name: say('unmatched.by_hand') });
	const outflow = '15 may 2026 · Checking · -$500.00 · TRANSF A CUENTA PROPIA';
	const other = form.getByLabel(say('unmatched.other_side'));
	await form.getByLabel(say('unmatched.one_side')).selectOption({ label: outflow });
	await other.selectOption({ label: outflow });
	await form.getByRole('button', { name: say('unmatched.pair_them') }).click();
	await expect(form.getByRole('alert')).toHaveText('choose two different transactions');
	await other.selectOption({ label: '15 may 2026 · Savings · $500.00 · FROM CHECKING' });
	await expect(form.getByRole('alert')).toHaveCount(0);
	await form.getByRole('button', { name: say('unmatched.pair_them') }).click();
	await expect(form.getByRole('status')).toHaveText(say('unmatched.paired'));
	await expect(made.getByRole('listitem')).toHaveCount(1);
});

test('coverage shows what was imported', async ({ context }) => {
	const page = await loggedIn(context);
	await page.goto('/coverage');
	const row = page.getByRole('row', { name: /Checking/ });
	await expect(row).toContainText('31 may 2026');
	await expect(row).toContainText('2 may 2026 – 31 may 2026');
	// Months behind any real today: shown as late, in words.
	await expect(row.locator('.chip')).toHaveText(say('delay.late'));
});

test('the dashboard matches the API and words what employers owe', async ({ context }) => {
	const page = await loggedIn(context);
	await loadEmployers(page, test.info().outputPath('employers.json'));
	await page.goto('/');
	await expect(
		page.getByRole('heading', { name: say('dashboard.title'), exact: true })
	).toBeVisible();
	await checkPayroll(page);
	await checkDashboard(page);
});

test('switches to English and remembers it after a reload', async ({ context }) => {
	const page = await loggedIn(context);
	await page.goto('/accounts');
	await page.getByLabel(say('language.label')).selectOption('en');
	const heading = page.getByRole('heading', { name: english('accounts.title'), exact: true });
	await expect(heading).toBeVisible();
	await expect(page.locator('html')).toHaveAttribute('lang', 'en');
	await page.reload();
	await expect(heading).toBeVisible();
	await expect(page.getByRole('link', { name: english('nav.summary'), exact: true })).toBeVisible();
	// Dates follow the language.
	await page.getByRole('link', { name: english('nav.coverage'), exact: true }).click();
	await expect(page.getByRole('row', { name: /Checking/ })).toContainText('May 31, 2026');
});

test('every page fits a 390 px screen', async ({ browser }) => {
	const phone = await browser.newContext({
		viewport: { width: 390, height: 844 },
		locale: 'es-PA'
	});
	const pages: [string, string][] = [
		['/login', say('app.name')],
		['/', say('dashboard.title')],
		['/import', say('import.title')],
		['/coverage', say('coverage.title')],
		['/accounts', say('accounts.title')],
		['/rules', say('rules.title')],
		['/uncategorized', say('uncategorized.title')],
		['/transfers', say('transfers.title')]
	];
	for (const [path, heading] of pages) {
		if (path !== '/login') await phone.addCookies(session.cookies);
		const page = await phone.newPage();
		await page.goto(path);
		await expect(page.getByRole('heading', { name: heading, exact: true })).toBeVisible();
		expect(await overflow(page), path).toBeLessThanOrEqual(0);
		if (path === '/') {
			// Every fold open (the month table, each employer's history): tables scroll in their own box.
			await page.getByLabel(say('period.from'), { exact: true }).fill('2026-05-01');
			await page.getByLabel(say('period.to'), { exact: true }).fill('2026-05-31');
			await expect(page.getByRole('region', { name: say('payroll.title') })).toBeVisible();
			await expect(page.getByText(say('chart.table'))).toBeVisible();
			for (const fold of await page.locator('summary').all()) await fold.click();
			await expect(page.locator('details[open]')).not.toHaveCount(0);
			expect(await overflow(page), 'dashboard with every fold open').toBeLessThanOrEqual(0);
		}
		if (path === '/import') {
			// With a card open and its transactions table expanded (the account made in the import test reads
			// it), the table scrolls in its own box.
			await page.getByLabel(say('import.choose')).setInputFiles(statement);
			const card = page.getByRole('article', { name: 'input.csv' });
			await card.getByText(say('file.show_other', { count: transactions })).click();
			await expect(card.getByRole('table')).toBeVisible();
			expect(await overflow(page), 'import with a table open').toBeLessThanOrEqual(0);
		}
		await page.close();
	}
	await phone.close();
});
