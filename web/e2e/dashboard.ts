// The dashboard against the API: every number on screen is the report's, formatted by the browser itself.
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { expect, type Browser, type Cookie, type Page } from '@playwright/test';
import { say, type Key } from './messages.ts';

const period = { from: '2026-05-01', to: '2026-05-31' };

/** Two synthetic employers on the statement's account: one that pays with the 1,462.16 deposit of 6 May, one that
 * never paid. Employers load only through the server's command line. */
export async function loadEmployers(page: Page, file: string) {
	const me = await (await page.request.get('/api/v1/me')).json();
	const employer = (name: string, match: string, amount: number) => ({
		name,
		match,
		account: 'bac',
		min_amount_minor: 100000,
		amount_minor: amount,
		start: '2026-04'
	});
	const employers = [
		employer('Example Co', 'EJEMPLO', 146216),
		employer('Nobody Inc', 'NOBODY', 50000)
	];
	writeFileSync(file, JSON.stringify({ employers }));
	execFileSync(process.env.E2E_SERVER ?? '', [
		'payroll',
		'load',
		file,
		'--household',
		me.household_id
	]);
}

/** An amount as the page writes it, with a true minus sign; `signed` writes the sign of a movement even when it is
 *  positive. */
const money = (page: Page, minor: number, signed = false) =>
	page.evaluate(
		([m, sign]) =>
			new Intl.NumberFormat('es-PA', {
				style: 'currency',
				currency: 'USD',
				currencyDisplay: 'narrowSymbol',
				signDisplay: sign ? 'exceptZero' : 'auto'
			})
				.format(m / 100)
				.replace('-', '\u2212'),
		[minor, signed] as const
	);

const date = (page: Page, iso: string, options: Intl.DateTimeFormatOptions) =>
	page.evaluate(
		([d, o]) =>
			new Intl.DateTimeFormat('es-PA', { ...o, timeZone: 'UTC' }).format(
				new Date(`${d}T00:00:00Z`)
			),
		[iso, options] as const
	);

type Item = { id: string | null; spending: number };

async function ranking(
	page: Page,
	title: Key,
	items: Item[],
	names: Map<string, string>,
	none: Key
) {
	const rows = page.getByRole('list', { name: say(title) }).getByRole('listitem');
	await expect(rows).toHaveCount(items.length);
	for (const [i, item] of items.entries()) {
		await expect(rows.nth(i)).toContainText(item.id ? (names.get(item.id) ?? '?') : say(none));
		await expect(rows.nth(i)).toContainText(await money(page, item.spending));
	}
}

export async function checkDashboard(page: Page) {
	const api = async (path: string) => (await page.request.get(path)).json();

	// «Por revisar» leads with the count the uncategorized page lists: one query for both (P3).
	const open = (await api('/api/v1/transactions?uncategorized=true&limit=1000')).items.length;
	const review = page.getByRole('region', { name: say('review.title') });
	await expect(review).toContainText(
		open === 1
			? say('review.uncategorized_one', { count: 1 })
			: open
				? say('review.uncategorized_other', { count: open })
				: say('review.all_categorized')
	);

	// The dates are only there under «Otro…».
	await expect(page.getByLabel(say('period.from'), { exact: true })).toHaveCount(0);
	await page.getByRole('button', { name: say('dashboard.other'), exact: true }).click();
	await page.getByLabel(say('period.from'), { exact: true }).fill(period.from);
	await page.getByLabel(say('period.to'), { exact: true }).fill(period.to);
	const q = `from=${period.from}&to=${period.to}`;
	const [summary, monthly, categories, payees, categoryList, payeeList] = await Promise.all(
		[
			`/api/v1/summary?${q}`,
			`/api/v1/reports/monthly?${q}`,
			`/api/v1/reports/categories?${q}`,
			`/api/v1/reports/payees?${q}&limit=10`,
			'/api/v1/categories',
			'/api/v1/payees'
		].map(api)
	);

	// Tiles: the summary's totals, in the one currency the statement has.
	expect(summary.totals).toHaveLength(1);
	const totals = summary.totals[0];
	const tile = (key: Key) =>
		page
			.locator('.figures > div', { has: page.getByText(say(key), { exact: true }) })
			.locator('dd');
	await expect(tile('tiles.spending')).toHaveText(await money(page, totals.spending));
	await expect(tile('tiles.income')).toHaveText(await money(page, totals.income));
	await expect(tile('tiles.net')).toHaveText(
		await money(page, totals.income - totals.spending, true)
	);

	// The chart's table holds the month's report.
	const may = monthly.months.find((m: { month: string }) => m.month === '2026-05');
	await page.getByText(say('chart.table')).click();
	const name = await date(page, '2026-05-01', { month: 'long', year: 'numeric' });
	await expect(
		page.getByRole('row', { name: new RegExp(`^${name}`) }).getByRole('cell')
	).toHaveText([
		await money(page, may.income),
		await money(page, may.spending),
		await money(page, may.income - may.spending, true)
	]);

	const byId = (list: { items: { id: string; name: string }[] }) =>
		new Map(list.items.map((x) => [x.id, x.name]));
	const cats = categories.items.map((i: Item & { category_id: string | null }) => ({
		...i,
		id: i.category_id
	}));
	await ranking(page, 'ranking.categories', cats, byId(categoryList), 'ranking.no_category');
	const pays = payees.items.map((i: Item & { payee_id: string | null }) => ({
		...i,
		id: i.payee_id
	}));
	await ranking(page, 'ranking.payees', pays, byId(payeeList), 'ranking.no_payee');
}

export async function checkPayroll(page: Page) {
	// «Ver» on the summary's card goes to the payments card and takes the focus along: it stayed on the page (pass 29).
	const review = page.getByRole('region', { name: say('review.title') });
	await review.getByRole('link', { name: say('review.see') }).click();
	await expect(page.locator('#payroll-title')).toBeFocused();
	const today = await page.evaluate(() => {
		const now = new Date();
		const pad = (n: number) => String(n).padStart(2, '0');
		return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
	});
	const payroll = await (await page.request.get(`/api/v1/payroll?as_of=${today}`)).json();
	const section = page.getByRole('region', { name: say('payroll.title') });
	const [paid, nobody] = ['Example Co', 'Nobody Inc'].map((n) =>
		payroll.employers.find((e: { name: string }) => e.name === n)
	);

	// Worked by hand: the 6 May deposit settles April's second half; April's first and May's first are owed (due
	// on or before the account's last data, 31 May); from May's second half on there is no data yet.
	expect(paid.owed).toHaveLength(2);
	const card = section.getByRole('article', { name: 'Example Co' });
	await expect(card).toContainText(await money(page, paid.owed_total_minor));
	await expect(card).toContainText(say('payroll.owed_other', { count: 2 }));
	await expect(
		card.getByRole('list', { name: say('payroll.status.owed') }).getByRole('listitem')
	).toHaveCount(2);
	const noData = card.getByRole('paragraph').filter({ hasText: say('payroll.status.no_data') });
	await expect(noData).toBeVisible();
	await card.getByText(say('payroll.history'), { exact: true }).click();
	// Newest first, with three or more fortnights in a row without data as one row that counts them.
	const statuses: string[] = [];
	const runs: number[] = [];
	let run = 0;
	for (const { status } of [...paid.history.toReversed(), { status: 'end' }]) {
		if (status === 'no_data') {
			run += 1;
			continue;
		}
		if (run >= 3) runs.push(run);
		statuses.push(...Array(run >= 3 ? 1 : run).fill(say('payroll.status.no_data')));
		run = 0;
		if (status !== 'end') statuses.push(say(`payroll.status.${status}` as Key));
	}
	await expect(card.getByRole('table').locator('.chip')).toHaveText(statuses);
	expect(statuses).toContain(say('payroll.status.late'));
	expect(runs.length).toBeGreaterThan(0);
	for (const count of runs) {
		await expect(
			card.getByRole('rowheader', { name: say('payroll.fortnights_other', { count }) })
		).toBeVisible();
	}

	const start = await date(page, nobody.start, { day: 'numeric', month: 'short', year: 'numeric' });
	await expect(section.getByRole('article', { name: 'Nobody Inc' })).toContainText(
		say('payroll.no_deposits', { date: start })
	);
}

/** On a phone each month of the open month table is a card of figures: they end on one line, so their decimals line
 * up (pass 17: to the left they read ragged). The employers' history mixes words in, so only rows of figures count. */
export async function checkMonthFigures(page: Page) {
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
	const months = ends.filter((row) => row.length > 1);
	expect(months.length).toBeGreaterThan(0);
	for (const row of months) expect(new Set(row).size, String(row)).toBe(1);
}

/**
 * On a phone, a tap on the words of a waiting row opens its page. The row is one stretched link: a press that scaled
 * the link made it the box of its tap area, which shrank to the chevron, and a tap on the words opened nothing.
 */
export async function checkRowTap(browser: Browser, cookies: Cookie[]) {
	const phone = await browser.newContext({
		viewport: { width: 390, height: 844 },
		hasTouch: true,
		isMobile: true,
		locale: 'es-PA'
	});
	await phone.addCookies(cookies);
	const page = await phone.newPage();
	await page.goto('/');
	const row = page.locator('.review .row:has(a)').first();
	await expect(row).toBeVisible();
	const href = await row.locator('a').getAttribute('href');
	const box = await row.boundingBox();
	if (!box || !href) throw new Error('no waiting row');
	await page.touchscreen.tap(box.x + box.width * 0.4, box.y + box.height / 2);
	await expect(page).toHaveURL(new URL(href, page.url()).href);
	await phone.close();
}
