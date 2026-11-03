// Rules moved and deleted through the browser, against a real server (see run.sh): the focus follows the rule and
// never falls to the page, and the page lands where the person can see what happened. Its own user.
import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import { watchDrops } from './focus.ts';
import { say } from './messages.ts';
import { makeRules } from './rules.ts';
import { still } from './scroll.ts';
import { totp } from './totp.ts';

// A TOTP code is accepted once, so the run logs in once and the later tests reuse that session.
test.describe.configure({ mode: 'serial' });
test.use({ locale: 'es-PA' });
let session: Awaited<ReturnType<BrowserContext['storageState']>>;

test.beforeAll(async ({ browser }) => {
	const secret = process.env.E2E_RULES_SECRET ?? '';
	expect(secret, 'run through e2e/run.sh').not.toBe('');
	const page = await browser.newPage({ locale: 'es-PA' });
	await page.goto('/rules');
	await page.getByLabel(say('login.email')).fill(process.env.E2E_RULES_EMAIL ?? '');
	await page.getByLabel(say('login.password')).fill(process.env.E2E_RULES_PASSWORD ?? '');
	await page.getByLabel(say('login.code')).fill(totp(secret));
	await page.getByRole('button', { name: say('login.title') }).click();
	await expect(page.getByRole('heading', { name: say('rules.title'), exact: true })).toBeVisible();
	session = await page.context().storageState();
	await page.close();
});

async function loggedIn(context: BrowserContext): Promise<Page> {
	await context.addCookies(session.cookies);
	return context.newPage();
}

test('a rule moved down keeps the focus on its buttons, never on the page', async ({ context }) => {
	const page = await loggedIn(context);
	const made = await makeRules(page, ['Down A', 'Down B', 'Down C']);
	await page.goto('/rules');
	const order = () => page.locator('ol.rules h2').allInnerTexts();
	const down = page.locator(`[data-move="${made[0]}-down"]`);
	await down.focus();
	const drops = await watchDrops(page);
	// The row going down is the one the list takes out and puts back, with the focused button in it (pass 20).
	await page.keyboard.press('Enter');
	await expect
		.poll(async () => {
			const now = await order();
			return now.indexOf('Down A') === now.indexOf('Down B') + 1;
		})
		.toBe(true);
	await expect(down).toBeFocused();
	// Once last it has no «Bajar»: the focus goes to its «Subir», straight.
	await page.keyboard.press('Enter');
	await expect(down).toHaveCount(0);
	await expect(page.locator(`[data-move="${made[0]}-up"]`)).toBeFocused();
	expect(await drops()).toEqual([]);
	for (const id of made) expect((await page.request.delete(`/api/v1/rules/${id}`)).ok()).toBe(true);
});

test('a rule moved up keeps its focused button on screen', async ({ context }) => {
	const page = await loggedIn(context);
	await page.setViewportSize({ width: 1280, height: 800 });
	const made = await makeRules(page, ['Moved A', 'Moved B', 'Moved C', 'Moved D']);
	await page.goto('/rules');
	// A rule near the top of the screen glides up after its button got the focus: the page has to follow it, or the
	// focus ends above the screen (pass 7, y = -144).
	const up = page.locator(`[data-move="${made[2]}-up"]`);
	await up.evaluate((e) => scrollBy(0, e.getBoundingClientRect().top - 90));
	await up.focus();
	const drops = await watchDrops(page);
	const reloaded = page.waitForResponse(
		(r) => r.url().includes('/api/v1/rules') && r.request().method() === 'GET'
	);
	await page.keyboard.press('Enter');
	await reloaded;
	await page.evaluate(
		() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)))
	);
	await up.evaluate((e) =>
		Promise.all(
			e
				.closest('li')!
				.getAnimations()
				.map((a) => a.finished.catch(() => undefined))
		)
	);
	await expect(up).toBeFocused();
	expect(await drops()).toEqual([]);
	await expect
		.poll(() =>
			up.evaluate(
				(e) => e.getBoundingClientRect().top >= 0 && e.getBoundingClientRect().bottom <= innerHeight
			)
		)
		.toBe(true);
	for (const id of made) expect((await page.request.delete(`/api/v1/rules/${id}`)).ok()).toBe(true);
});

test('on a phone a rule moved up glides, then lands with no line across the bar', async ({
	context
}) => {
	const page = await loggedIn(context);
	await page.setViewportSize({ width: 390, height: 844 });
	const made = await makeRules(page, ['Landing A', 'Landing B']);
	await page.goto('/rules');
	// The browser's scroll anchoring kept the moved rule still by moving the page in one frame, its title across the
	// top bar, and the page then landed with a line flush under the bar (pass 13: 365 → 147, 300 → 17).
	const up = page.locator(`[data-move="${made[1]}-up"]`);
	await up.evaluate((e) => scrollBy(0, e.getBoundingClientRect().top - 210));
	const start = await still(page);
	// Clicked where it stands, as a person would: once the page moved at the click (630 → 97), as a Playwright retry does.
	await up.click({ force: true });
	// The rules have swapped once the one above, now last, has no «Bajar»: the page has not moved yet.
	await expect(page.locator(`[data-move="${made[0]}-down"]`)).toHaveCount(0);
	expect(await page.evaluate(() => scrollY)).toBe(start);
	const row = page.locator('.rules > li', { has: page.locator(`[data-move^="${made[1]}-"]`) });
	await row.evaluate((e) => Promise.all(e.getAnimations().map((a) => a.finished.catch(() => 0))));
	await still(page, 500);
	const cut = await page.evaluate(() => {
		const bar = document.querySelector('header.appbar')!.getBoundingClientRect().bottom;
		return [...document.querySelectorAll('main h2, main p')]
			.flatMap((e) => {
				const range = document.createRange();
				range.selectNodeContents(e);
				return [...range.getClientRects()];
			})
			.filter((r) => r.height > 2 && r.top < bar + 8 && r.bottom > bar).length;
	});
	expect(cut).toBe(0);
	for (const id of made) expect((await page.request.delete(`/api/v1/rules/${id}`)).ok()).toBe(true);
});

test('on a phone, deleting a rule leaves the page where it was', async ({ context }) => {
	const page = await loggedIn(context);
	await page.setViewportSize({ width: 390, height: 844 });
	const made = await makeRules(page, ['Delete A', 'Delete B']);
	// Each step read once the page stands still: the list loads and the confirm opens with their own motion.
	await page.goto('/rules');
	const ask = page.getByRole('button', { name: say('rules.delete_named', { name: 'Delete B' }) });
	await expect(ask).toBeVisible();
	await still(page);
	await ask.evaluate((e) => scrollBy(0, e.getBoundingClientRect().top - 400));
	await ask.click();
	const yes = page.getByRole('button', { name: say('rules.delete_it') });
	const start = await still(page);
	const drops = await watchDrops(page);
	await yes.click();
	await expect(page.getByRole('heading', { name: 'Delete B' })).toHaveCount(0);
	// The focus goes to the page's title, which a phone shows in its top bar: focused bare, the page jumped to its
	// start in one frame (pass 15).
	await expect(page.locator('main h1')).toBeFocused();
	expect(await drops()).toEqual([]);
	expect(await still(page)).toBe(start);
	expect((await page.request.delete(`/api/v1/rules/${made[0]}`)).ok()).toBe(true);
});

test('a rule under a deleted one waits in its place while that one fades', async ({ context }) => {
	const page = await loggedIn(context);
	const [above, under] = await makeRules(page, ['Wait A', 'Wait B']);
	await page.goto('/rules');
	const ask = page.getByRole('button', { name: say('rules.delete_named', { name: 'Wait A' }) });
	await expect(ask).toBeVisible();
	await ask.click();
	const yes = page.getByRole('button', { name: say('rules.delete_it') });
	await still(page);
	// Every frame while the deleted card fades: how far the card under it stands from where it stood. A bordered card
	// waited 2 px above its place, then slid on (pass 21: Svelte's flip scales the distance by the box without borders).
	type Frames = { moved: number[] };
	await page.evaluate(
		([gone, next]) => {
			const w = window as unknown as Frames;
			const leaving = document.querySelector(`[data-rule="${gone}"]`)!;
			const watched = document.querySelector(`[data-rule="${next}"]`)!;
			const start = watched.getBoundingClientRect().top;
			w.moved = [];
			const step = () => {
				const fading = leaving.isConnected && Number(getComputedStyle(leaving).opacity) > 0.02;
				if (fading) w.moved.push(watched.getBoundingClientRect().top - start);
				if (leaving.isConnected) requestAnimationFrame(step);
			};
			requestAnimationFrame(step);
		},
		[above, under]
	);
	await yes.click();
	await expect(page.getByRole('heading', { name: 'Wait A' })).toHaveCount(0);
	const moved = await page.evaluate(() => (window as unknown as Frames).moved);
	expect(moved.length).toBeGreaterThan(3);
	expect(moved.filter((offset) => Math.abs(offset) > 0.5)).toEqual([]);
	expect((await page.request.delete(`/api/v1/rules/${under}`)).ok()).toBe(true);
});
