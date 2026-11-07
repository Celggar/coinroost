// Every page on a phone: nothing runs wider than the screen, and what is wide scrolls in its own box.
import {
	expect,
	type Browser,
	type BrowserContext,
	type Locator,
	type Page
} from '@playwright/test';
import { checkMonthFigures } from './dashboard.ts';
import { widestTabs } from './lines.ts';
import { say } from './messages.ts';

/** How far the page runs past the screen. */
export async function overflow(page: Page): Promise<number> {
	return page.evaluate(
		() => document.documentElement.scrollWidth - document.documentElement.clientWidth
	);
}

/** Each page at 390 px, signed in with `cookies` and with `statement`, of `transactions` rows, read by its account. */
export async function checkEveryPage(
	browser: Browser,
	cookies: Parameters<BrowserContext['addCookies']>[0],
	statement: Parameters<Locator['setInputFiles']>[0],
	transactions: number
) {
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
		['/categories', say('categories.title')],
		['/uncategorized', say('uncategorized.title')],
		['/transfers', say('transfers.title')]
	];
	for (const [path, heading] of pages) {
		if (path !== '/login') await phone.addCookies(cookies);
		const page = await phone.newPage();
		await page.goto(path);
		await expect(page.getByRole('heading', { name: heading, exact: true })).toBeVisible();
		expect(await overflow(page), path).toBeLessThanOrEqual(0);
		if (path === '/') {
			// Every fold open (the month table, each employer's history, the dates under «Otro…»): tables scroll in
			// their own box.
			await page.getByRole('button', { name: say('dashboard.other'), exact: true }).click();
			await page.getByLabel(say('period.from'), { exact: true }).fill('2026-05-01');
			await page.getByLabel(say('period.to'), { exact: true }).fill('2026-05-31');
			await expect(page.getByRole('region', { name: say('payroll.title') })).toBeVisible();
			await expect(page.getByText(say('chart.table'))).toBeVisible();
			for (const fold of await page.locator('summary').all()) await fold.click();
			await expect(page.locator('details[open]')).not.toHaveCount(0);
			expect(await overflow(page), 'dashboard with every fold open').toBeLessThanOrEqual(0);
			await checkMonthFigures(page);
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
		if (path === '/uncategorized') {
			// At 360 px each tab's label holds on one line within its padding, even with the widest count («99+»):
			// «Sin categoría 16» broke after «Sin» and «Transferencias 2» ran into its padding (pass 22 sweep).
			await page.setViewportSize({ width: 360, height: 800 });
			for (const tab of await widestTabs(page)) expect(tab).toEqual({ lines: 1, spill: 0 });
		}
		await page.close();
	}
	await phone.close();
}
