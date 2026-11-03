// The app through the browser in Spanish, against a real server (see run.sh): log in, create an account, preview
// and import a statement, import it again, sort, pair transfers, read the coverage and the dashboard, and switch
// to English.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type BrowserContext, type Locator, type Page } from '@playwright/test';
import { checkDashboard, checkPayroll, loadEmployers } from './dashboard.ts';
import { watchDrops } from './focus.ts';
import { english, say } from './messages.ts';
import { still } from './scroll.ts';
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
	// A card's file makes the account a credit card; a file that reads either kind gives back the chosen one.
	const kind = page.getByLabel(say('accounts.kind'));
	await page.getByLabel(say('accounts.bank_file')).selectOption('bac-card');
	await expect(kind).toHaveValue('credit_card');
	await page.getByLabel(say('accounts.bank_file')).selectOption('bac-account');
	await expect(kind).toHaveValue('checking');
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
	const account = other.getByLabel(say('file.account'));
	await account.selectOption({ label: 'Checking' });
	// The reason goes under «Cuenta», tied to it, in place of the note that no longer holds; the way on to an account
	// that reads the file stays (pass 10: only «Quitar» was left).
	await expect(account).toHaveAttribute('aria-invalid', 'true');
	await expect(other.getByText(say('file.not_found'))).toHaveCount(0);
	const reason = other.locator('.field-error');
	await expect(reason).not.toBeEmpty();
	const described = (await account.getAttribute('aria-describedby'))?.split(' ') ?? [];
	expect(described).toContain(await reason.getAttribute('id'));
	await expect(other.getByRole('link', { name: say('file.open_accounts') })).toBeVisible();
	// In the page's language: the text of the problem's code, never the API's English detail.
	const why = (await reason.textContent())?.trim();
	expect([say('problem.file_unreadable'), say('problem.file_missing_columns')]).toContain(why);

	const drops = await watchDrops(page);
	await card
		.getByRole('button', { name: say('file.import_other', { count: transactions }) })
		.click();
	// The page's notice says what the import did, once (FD8); the card keeps «imported» as its own state.
	const added = say('file.added_other', { count: transactions });
	await expect(page.getByRole('status').filter({ hasText: added })).toHaveText(added);
	await expect(card.getByText(added)).toHaveCount(0);
	await expect(card.getByText(say('file.imported'))).toBeFocused();
	expect(await drops()).toEqual([]);
	// The line's ring goes around its words: a 48 px box left 12 px above them and 2 px beside them (pass 21).
	const above = await card.getByText(say('file.imported')).evaluate((e) => {
		const words = document.createRange();
		words.selectNodeContents(e.lastChild!);
		return words.getBoundingClientRect().top - e.getBoundingClientRect().top;
	});
	expect(above).toBeLessThanOrEqual(4);
	// The card is the record of that import: it says what it added, like the notice, not «0 new».
	await expect(count(card, say('file.added'))).toHaveText(String(transactions));
	await expect(count(card, say('file.already'))).toHaveText('0');

	// The same file again: everything is already there, said once, with no import button to press.
	await page.getByLabel(say('import.choose')).setInputFiles(statement);
	const again = page.getByRole('article', { name: 'input.csv' }).nth(1);
	await expect(count(again, say('file.new'))).toHaveText('0');
	await expect(count(again, say('file.already'))).toHaveText(String(transactions));
	await expect(again.getByRole('status')).toHaveText(say('file.nothing_new'));
	await expect(
		again.getByRole('button', { name: say('file.import_other', { count: 0 }) })
	).toHaveCount(0);

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

	// Imported before the rule existed, so nothing is sorted yet: choose one by hand. «Revisar» opens on its first
	// tab, «Sin categoría».
	await page.getByRole('link', { name: say('nav.review'), exact: true }).click();
	await expect(
		page.getByRole('link', { name: say('nav.uncategorized'), exact: true })
	).toHaveAttribute('aria-current', 'page');
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
	// The answer lands whole, down to the button that applies it, above the page's bottom room: a whole field at the
	// top once won over it, and the button stood half below the screen (pass 14).
	const send = page.getByRole('button', { name: say('changes.apply_one') });
	await expect
		.poll(() =>
			send.evaluate((e) => {
				const room = parseFloat(getComputedStyle(document.documentElement).scrollPaddingBottom);
				return e.getBoundingClientRect().bottom <= innerHeight - room + 0.5;
			})
		)
		.toBe(true);
	// Revisar's count (read from the words its badge stands for) drops with the sorted transaction, without a page change.
	const reviewCount = async () =>
		Number((await page.locator('#rail-review').textContent())?.match(/\d+/)?.[0] ?? 0);
	const before = await reviewCount();
	expect(before).toBeGreaterThan(0);
	await page.getByRole('button', { name: say('changes.apply_one') }).click();
	const applied = say('changes.applied_one');
	await expect(page.getByRole('status').filter({ hasText: applied })).toHaveText(applied);
	await expect(page.getByText(say('changes.nothing'))).toBeVisible();
	await expect.poll(reviewCount).toBe(before - 1);

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
	await expect(waiting).toContainText(
		`Checking (15 may 2026) ${say('transfer.to')} Savings (15 may 2026)`
	);
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

	// By hand: «Otro lado» never offers the leg chosen in «Un lado» (pass 19). An API refusal (two legs that do not
	// cancel) shows in Spanish, never its English detail, under «Otro lado», marked and focused; sent again, they pair.
	const form = page.getByRole('form', { name: say('unmatched.by_hand') });
	const outflow = '15 may 2026 · Checking · −$500.00 · TRANSF A CUENTA PROPIA';
	const other = form.getByLabel(say('unmatched.other_side'));
	await form.getByLabel(say('unmatched.one_side')).selectOption({ label: outflow });
	await expect(other.getByRole('option', { name: outflow })).toBeDisabled();
	await other.selectOption({ label: '15 may 2026 · Savings · +$500.00 · FROM CHECKING' });
	const refusal = { status: 422, code: 'transfer_not_balanced', detail: 'they do not cancel' };
	await page.route(
		'**/api/v1/transfers',
		(route) => route.fulfill({ status: 422, json: refusal }),
		{ times: 1 }
	);
	await form.getByRole('button', { name: say('unmatched.pair_them') }).click();
	await expect(other).toHaveAttribute('aria-invalid', 'true');
	await expect(other).toBeFocused();
	const reason = form.locator(`[id="${await other.getAttribute('aria-describedby')}"]`);
	await expect(reason).toHaveText(say('problem.transfer_not_balanced'));
	await form.getByRole('button', { name: say('unmatched.pair_them') }).click();
	await expect(page.getByRole('status')).toHaveText(say('unmatched.paired'));
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

test('on a phone, changes applied at the page end leave the page still as it shortens', async ({
	context
}) => {
	const page = await loggedIn(context);
	await page.setViewportSize({ width: 390, height: 844 });
	const categories: { id: string; kind: string }[] = (
		await (await page.request.get('/api/v1/categories')).json()
	).items;
	const response = await page.request.post('/api/v1/rules', {
		data: {
			name: 'Landing C',
			when: { description_contains: 'INTERESES' },
			then: { category_id: categories.find((c) => c.kind !== 'transfer')!.id }
		}
	});
	expect(response.ok()).toBe(true);
	const id = (await response.json()).id;
	await page.goto('/rules');
	await page.getByRole('button', { name: say('changes.check') }).click();
	const apply = page.getByRole('button', { name: say('changes.apply_one') });
	await apply.scrollIntoViewIfNeeded();
	// Every frame from the click on: the applied change left at the page's end, the page shrank, and the browser pulled
	// it back 171 px in that same frame (pass 14).
	type Frames = { frames: [number, number][] };
	await page.evaluate(() => {
		const w = window as unknown as Frames;
		w.frames = [];
		const step = () => {
			w.frames.push([scrollY, document.documentElement.scrollHeight]);
			if (w.frames.length < 150) requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	});
	await apply.click();
	await page.waitForFunction(() => (window as unknown as Frames).frames.length >= 150);
	const frames = await page.evaluate(() => (window as unknown as Frames).frames);
	const pulled = frames
		.slice(1)
		.filter(([y, height], i) => height < frames[i][1] && y !== frames[i][0]);
	expect(pulled).toEqual([]);
	await expect(page.getByText(say('changes.nothing'))).toBeInViewport();
	expect((await page.request.delete(`/api/v1/rules/${id}`)).ok()).toBe(true);
});

test('on a short phone, a file the account cannot read says why above the bottom bar', async ({
	context
}) => {
	const page = await loggedIn(context);
	await page.setViewportSize({ width: 390, height: 600 });
	await page.goto('/import');
	await page
		.getByLabel(say('import.choose'))
		.setInputFiles([
			statement,
			{ name: 'notes.txt', mimeType: 'text/plain', buffer: Buffer.from('just, some\nwords') }
		]);
	const card = page.getByRole('article', { name: 'notes.txt' });
	await expect(card.getByText(say('file.not_found'))).toBeVisible();
	await page.evaluate(() => scrollTo(0, document.documentElement.scrollHeight));
	await still(page);
	// From the page's end, the error opened under the account and the page stayed: its last line sat under the
	// bottom bar while the account kept the focus above it (pass 21).
	const account = card.getByLabel(say('file.account'));
	await account.focus();
	await account.selectOption({ label: 'Checking' });
	const reason = card.locator('.field-error');
	await expect(reason).not.toBeEmpty();
	// Once the card has grown to hold it, the page moves: how far the error's end stands below the bar's top.
	const under = () =>
		reason.evaluate(
			(e) =>
				e.getBoundingClientRect().bottom -
				document.querySelector('nav.bar')!.getBoundingClientRect().top
		);
	await expect.poll(under).toBeLessThanOrEqual(0);
	await expect(account).toBeInViewport();
});

test('a failed recovery status offers no codes to make', async ({ context }) => {
	const page = await loggedIn(context);
	// «Crear códigos» while the status is unknown would replace codes the person may already have.
	await page.route('**/api/v1/me/recovery-codes', (route) =>
		route.request().method() === 'GET'
			? route.fulfill({
					status: 503,
					contentType: 'application/problem+json',
					body: '{"status":503}'
				})
			: route.continue()
	);
	await page.goto('/security');
	const codes = page.getByRole('region', { name: say('security.codes_title') });
	await expect(codes.getByRole('alert')).toBeVisible();
	await expect(
		codes.getByRole('button', { name: say('security.confirm'), exact: true })
	).toHaveCount(0);
	await expect(
		codes.getByRole('button', { name: say('security.regenerate'), exact: true })
	).toHaveCount(0);
	await page.unroute('**/api/v1/me/recovery-codes');
	await codes.getByRole('button', { name: say('app.try_again') }).click();
	await expect(
		codes.getByRole('button', { name: say('security.confirm'), exact: true })
	).toBeVisible();
});

test('a failed money summary on Transfers offers to try again', async ({ context }) => {
	const page = await loggedIn(context);
	// Its alert says to try again in a moment: without a button the only way back was changing a date.
	const summary = (url: URL) => url.pathname === '/api/v1/summary';
	await page.route(summary, (route) =>
		route.fulfill({
			status: 503,
			contentType: 'application/problem+json',
			body: '{"status":503}'
		})
	);
	await page.goto('/transfers');
	const card = page.getByRole('region', { name: say('summary.title') });
	await expect(card.getByRole('alert')).toBeVisible();
	await page.unroute(summary);
	const drops = await watchDrops(page);
	await card.getByRole('button', { name: say('app.try_again') }).click();
	await expect(card.getByRole('alert')).toHaveCount(0);
	await expect(
		card
			.getByText(say('summary.spending'))
			.or(card.getByText(say('summary.nothing')))
			.first()
	).toBeVisible();
	await expect(page.locator('#summary-title')).toBeFocused();
	expect(await drops()).toEqual([]);
});

test('switches to English and remembers it after a reload', async ({ context }) => {
	const page = await loggedIn(context);
	await page.goto('/accounts');
	await page.getByRole('radio', { name: 'English' }).check();
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
		['/login', say('login.title')],
		['/setup', say('setup.title')],
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
			// Each month's figures end on one line, so their decimals line up (pass 17: to the left they read ragged).
			const ends = await page
				.locator('details[open] table.rows-on-phone tbody tr')
				.evaluateAll((rows) =>
					rows.map((row) =>
						[...row.querySelectorAll('td.number')].map((cell) => {
							const range = document.createRange();
							range.selectNodeContents(cell);
							return Math.round([...range.getClientRects()].at(-1)!.right);
						})
					)
				);
			// The month table's cards (the employers' history mixes words in).
			const months = ends.filter((row) => row.length > 1);
			expect(months.length).toBeGreaterThan(0);
			for (const row of months) expect(new Set(row).size, String(row)).toBe(1);
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
