// The dashboard against the API: every number on screen is the report's, formatted by the browser itself.
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { expect, type Browser, type Cookie, type Page } from '@playwright/test';
import { say, type Key } from './messages.ts';
import { still } from './scroll.ts';

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

/** An account brought up to date on another device: the answer reaches the open summary, its row shrinks and what
 * follows moves. Under reduced motion nothing may slide, so it fades in at its new place. */
export async function checkLateRowFades(page: Page) {
	const name = 'An account with a name so long that its detail takes several lines on a phone';
	let late = true;
	let release = () => {};
	const released = new Promise<void>((done) => (release = done));
	await page.route('**/api/v1/coverage*', async (route) => {
		const answer = await route.fetch();
		const body = await answer.json();
		if (!late) await released;
		const [first, ...rest] = body.items;
		const items = [{ ...first, name, latest: '2026-01-31', delay: late ? 'late' : 'up_to_date' }];
		await route.fulfill({
			response: answer,
			json: {
				...body,
				items: [...items, ...rest.map((x: object) => ({ ...x, delay: 'up_to_date' }))]
			}
		});
	});
	await page.setViewportSize({ width: 390, height: 844 });
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.goto('/');
	const review = page.getByRole('region', { name: say('review.title') });
	await expect(review.getByText(name)).toBeVisible();
	// Both refreshes wait, so the summary is back with the old answer when the new one lands.
	late = false;
	await page.locator('a[href="/accounts"]:visible').first().click();
	await page.waitForURL('**/accounts');
	await page.locator('a[href="/"]:visible').first().click();
	await expect(review.getByText(name)).toBeVisible();
	await watchWhatFollows(page, 'section:has(#review-title)');
	release();
	await expect(review.getByText(name)).toBeHidden();
	await expectFadedIn(page, 1);
}

type Frames = { seen: { top: number; opacity: number }[] };

/** Records, frame by frame, where what follows `selector` stands and how much of it shows. */
async function watchWhatFollows(page: Page, selector: string) {
	await page.evaluate((selector) => {
		const w = window as unknown as Frames;
		const next = document.querySelector(selector)?.nextElementSibling;
		if (!next) throw new Error('nothing follows');
		w.seen = [];
		const step = () => {
			// With the page's fade: it may be what it comes through.
			const of = (e: Element | null) => Number(e ? getComputedStyle(e).opacity : 1);
			const opacity = of(next) * of(next.closest('main'));
			w.seen.push({ top: next.getBoundingClientRect().top, opacity });
			if (w.seen.length < 120) requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	}, selector);
}

/** It moved at once, up (1) or down (-1), and stood at its new place only through a fade. */
async function expectFadedIn(page: Page, way: 1 | -1) {
	await page.waitForFunction(() => (window as unknown as Frames).seen.length >= 120);
	const seen = await page.evaluate(() => (window as unknown as Frames).seen);
	const steps = seen.slice(1).map((frame, i) => (seen[i].top - frame.top) * way);
	const drop = Math.max(...steps);
	expect(drop).toBeGreaterThan(4);
	expect(seen[steps.indexOf(drop) + 1].opacity).toBeLessThan(0.9);
	expect(seen.at(-1)?.opacity).toBe(1);
}

/** «Otro…» under reduced motion: the dates take their room and give it back at once, and what follows the period fades
 * in at its new place both ways. */
export async function checkOtherFades(page: Page) {
	await page.setViewportSize({ width: 390, height: 844 });
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.goto('/');
	const other = page.getByRole('button', { name: say('dashboard.other'), exact: true });
	await expect(other).toBeVisible();
	await watchWhatFollows(page, '.period');
	await other.click();
	await expectFadedIn(page, -1);
	await watchWhatFollows(page, '.period');
	await page.getByRole('button', { name: say('dashboard.this_year'), exact: true }).click();
	await expectFadedIn(page, 1);
}

export async function checkPayroll(page: Page) {
	// «Ver» on the summary's card glides to the payments card and takes the focus along: it stayed on the page (pass
	// 29), then got there in one frame, 1039 px at once (pass 31).
	const review = page.getByRole('region', { name: say('review.title') });
	type Frames = { ys: number[] };
	await page.evaluate(() => {
		const w = window as unknown as Frames;
		w.ys = [];
		const step = () => {
			w.ys.push(scrollY);
			if (w.ys.length < 90) requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	});
	await review.getByRole('link', { name: say('review.see') }).click();
	await expect(page.locator('#payroll-title')).toBeFocused();
	await page.waitForFunction(() => (window as unknown as Frames).ys.length >= 90);
	const ys = await page.evaluate(() => (window as unknown as Frames).ys);
	const travel = Math.max(...ys) - Math.min(...ys);
	expect(travel).toBeGreaterThan(0);
	const steps = ys.slice(1).map((y, i) => Math.abs(y - ys[i]));
	expect(Math.max(...steps)).toBeLessThan(travel / 2);
	// It lands on the card, not on a title at the screen's foot with the card under it.
	const landed = await page.locator('#payroll').boundingBox();
	const room = page.viewportSize()?.height ?? 0;
	if (!landed) throw new Error('no payments card');
	const shown = Math.min(landed.y + landed.height, room) - Math.max(landed.y, 0);
	expect(shown).toBeGreaterThan(Math.min(landed.height, room) / 2);
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

/** The card's opacity in the first frame it is on the page. It came whole and opaque in one frame: its fade sat inside
 * the block that made it, where a local transition does not play (pass 21). */
export async function checkLatePayroll(page: Page) {
	await page.route('**/api/v1/payroll*', async (route) => {
		await new Promise((done) => setTimeout(done, 1500));
		await route.continue();
	});
	type First = { opacity: number | null };
	await page.addInitScript(() => {
		const w = window as unknown as First;
		w.opacity = null;
		const step = () => {
			const card = document.querySelector('[aria-labelledby="payroll-title"]');
			if (card) w.opacity = Number(getComputedStyle(card).opacity);
			else requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	});
	await page.goto('/');
	await page.waitForFunction(() => (window as unknown as First).opacity !== null);
	expect(await page.evaluate(() => (window as unknown as First).opacity)).toBeLessThan(0.5);
}

/** Where the card stands in the first frame it is on the page. With a slow answer it stood alone under «Cargando…», and
 * the top coming in dropped it 1211 px in one frame (pass 36). */
export async function checkPayrollWithTop(page: Page) {
	await page.route('**/api/v1/coverage*', async (route) => {
		await new Promise((done) => setTimeout(done, 800));
		await route.continue();
	});
	type First = { at: number | null };
	await page.addInitScript(() => {
		const w = window as unknown as First;
		w.at = null;
		const step = () => {
			const card = document.querySelector('[aria-labelledby="payroll-title"]');
			if (card) w.at = card.getBoundingClientRect().top + scrollY;
			else requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	});
	await page.goto('/');
	await page.waitForFunction(() => (window as unknown as First).at !== null);
	await expect(page.locator('.reports')).toBeVisible();
	await still(page);
	const first = await page.evaluate(() => (window as unknown as First).at!);
	const settled = await page
		.locator('[aria-labelledby="payroll-title"]')
		.evaluate((card) => card.getBoundingClientRect().top + scrollY);
	expect(Math.abs(settled - first)).toBeLessThan(2);
}

/** On a phone, changes applied at the page's end leave the page still as it shortens. */
export async function checkAppliedAtEnd(page: Page) {
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
	await page.getByRole('button', { name: say('changes.open') }).click();
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
}

/** The tab as the browser reports it: Playwright has no way to hide one, so the two properties stand in for it. */
const showTab = (page: Page, shown: boolean) =>
	page.evaluate((visible) => {
		const state = { configurable: true };
		Object.defineProperty(document, 'hidden', { ...state, get: () => !visible });
		Object.defineProperty(document, 'visibilityState', {
			...state,
			get: () => (visible ? 'visible' : 'hidden')
		});
		document.dispatchEvent(new Event('visibilitychange'));
	}, shown);

/** Sorted in another tab, the summary went on saying «16 transacciones sin categoría» (pass 37). */
export async function checkBackOnTab(page: Page) {
	const waiting: { id: string }[] = (
		await (await page.request.get('/api/v1/transactions?uncategorized=true&limit=1000')).json()
	).items;
	expect(waiting.length).toBeGreaterThan(2);
	const row = (count: number) => page.getByText(say('review.uncategorized_other', { count }));
	let asked = 0;
	page.on('request', (request) => {
		if (new URL(request.url()).pathname === '/api/v1/summary') asked += 1;
	});
	await page.goto('/');
	await expect(row(waiting.length)).toBeVisible();
	await still(page);

	// Back with no news: nothing is asked for the page, nothing dims, nothing plays.
	asked = 0;
	type Frames = { frames: number; moved: number; drops: number[] };
	const watch = () =>
		page.evaluate(() => {
			const w = window as unknown as Frames;
			Object.assign(w, { frames: 0, moved: 0, drops: [] });
			let last: number | null = null;
			const step = () => {
				const running = document
					.getAnimations()
					.filter(
						(a) => a.effect?.getTiming().iterations !== Infinity && a.playState === 'running'
					);
				if (running.length || document.querySelector('.stale')) w.moved += 1;
				const top = document.querySelector('#payroll')?.getBoundingClientRect().top ?? 0;
				if (last !== null) w.drops.push(Math.abs(top + scrollY - last));
				last = top + scrollY;
				if (++w.frames < 60) requestAnimationFrame(step);
			};
			requestAnimationFrame(step);
		});
	await showTab(page, false);
	await watch();
	await showTab(page, true);
	await page.waitForFunction(() => (window as unknown as Frames).frames >= 60);
	expect(await page.evaluate(() => (window as unknown as Frames).moved)).toBe(0);
	expect(asked).toBe(0);

	// Something moved while away: the row, the badge and the reports follow, the reports asked again.
	const categories: { id: string; kind: string }[] = (
		await (await page.request.get('/api/v1/categories')).json()
	).items;
	const category = categories.find((c) => c.kind !== 'transfer')!.id;
	const sort = (id: string) =>
		page.request.patch(`/api/v1/transactions/${id}`, { data: { category_id: category } });
	expect((await sort(waiting[0].id)).ok()).toBe(true);
	await showTab(page, false);
	await watch();
	await showTab(page, true);
	await expect(row(waiting.length - 1)).toBeVisible();
	await expect.poll(() => asked).toBeGreaterThan(0);
	await page.waitForFunction(() => (window as unknown as Frames).frames >= 60);
	const drop = () => page.evaluate(() => Math.max(...(window as unknown as Frames).drops));

	// All sorted: the row turns «al día», on a phone where it shrinks most, and what follows glides.
	await page.setViewportSize({ width: 390, height: 844 });
	await still(page);
	for (const { id } of waiting.slice(1)) {
		expect((await sort(id)).ok()).toBe(true);
	}
	// Its link taken with it, the focus fell to the page (pass 38).
	await page.getByRole('link', { name: say('review.categorize') }).focus();
	await showTab(page, false);
	await watch();
	await showTab(page, true);
	await expect(page.getByText(say('review.all_categorized'))).toBeVisible();
	await page.waitForFunction(() => (window as unknown as Frames).frames >= 60);
	expect(await page.evaluate(() => document.activeElement?.tagName)).not.toBe('BODY');
	expect(await drop()).toBeLessThan(50);
	for (const { id } of waiting) {
		const sent = await page.request.patch(`/api/v1/transactions/${id}`, {
			data: { category_id: null }
		});
		expect(sent.ok()).toBe(true);
	}

	// A list that moved opens afresh: changed in place it left rows with the focus in them and jumped from its end (pass 38).
	await page.setViewportSize({ width: 1280, height: 800 });
	await page.goto('/uncategorized');
	const all = page.locator('label', { hasText: say('uncategorized.all') });
	await expect(all).toContainText(String(waiting.length));
	expect((await sort(waiting[0].id)).ok()).toBe(true);
	await showTab(page, false);
	const loaded = page.waitForEvent('load');
	await showTab(page, true);
	await loaded;
	await expect(all).toContainText(String(waiting.length - 1));
	const undone = await page.request.patch(`/api/v1/transactions/${waiting[0].id}`, {
		data: { category_id: null }
	});
	expect(undone.ok()).toBe(true);
}

/** A row leaving a list that lends it its columns keeps its line: out of the flow it broke in three, at full opacity, on
 * every choice (pass 38). */
export async function checkRowLeaves(page: Page) {
	await page.setViewportSize({ width: 1280, height: 800 });
	await page.goto('/uncategorized');
	const row = page.locator('.queue > li').first();
	const select = row.getByRole('combobox');
	const id = (await select.getAttribute('id'))!.replace('category-', '');
	type Frames = { drift: number[] };
	await row.evaluate((li) => {
		const w = window as unknown as Frames;
		const money = li.querySelector('.money')!;
		w.drift = [];
		const step = () => {
			if (!li.isConnected) return;
			w.drift.push(money.getBoundingClientRect().top - li.getBoundingClientRect().top);
			requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	});
	await select.selectOption({ index: 1 });
	// A chosen row stays until the next choice, and leaves then.
	const undo = page.locator(`[id="undo-${id}"]`);
	await expect(undo).toBeVisible();
	// Its tint first: a row still changing colour stays in the flow as it leaves, and nothing would be tested.
	await row.evaluate((li) => Promise.all(li.getAnimations().map((tint) => tint.finished)));
	const next = page.locator('.queue > li').nth(1).getByRole('combobox');
	const other = (await next.getAttribute('id'))!.replace('category-', '');
	await next.selectOption({ index: 1 });
	await expect(undo).toHaveCount(0);
	const drift = await page.evaluate(() => (window as unknown as Frames).drift);
	expect(drift.length).toBeGreaterThan(3);
	expect(Math.max(...drift) - Math.min(...drift)).toBeLessThan(2);
	for (const chosen of [id, other]) {
		const undone = await page.request.patch(`/api/v1/transactions/${chosen}`, {
			data: { category_id: null }
		});
		expect(undone.ok()).toBe(true);
	}
}
