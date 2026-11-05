// The queue where the page ends and under a notice, through the browser, against a real server (see run.sh): its own
// user, as the categorizing spec has reached its size.
import { expect, test } from '@playwright/test';
import { watchDrops } from './focus.ts';
import { expectPageFollows, watchRows } from './frames.ts';
import { say } from './messages.ts';
import { answered, fill, food, logIn, queue, rows, session, sortAll, spend } from './queue.ts';
import { still } from './scroll.ts';

// A TOTP code is accepted once, so the run logs in once and the later tests reuse that session.
test.describe.configure({ mode: 'serial' });
test.use({ locale: 'es-PA' });
// Long enough for a row to take a line more with its category, which it gives back with the offer.
const LONG = 'A category whose name takes a whole line of its own, and on a phone some more';

test.beforeAll(async ({ browser }) => {
	await logIn(browser, 'QUEUE');
	const context = await browser.newContext();
	await context.addCookies(session.cookies);
	const data = { name: LONG, kind: 'expense' };
	expect((await context.request.post('/api/v1/categories', { data })).ok()).toBe(true);
	await context.close();
});

for (const key of ['uncategorized.undo', 'uncategorized.only_this'] as const) {
	test(`under reduced motion «${say(key)}» at the page end goes up only through a fade`, async ({
		context
	}) => {
		for (const width of [390, 1280]) {
			const names = 'ABCDEFGH'.split('').map((name) => `${key} ${width} ${name}`);
			await fill(
				context,
				names.map((name) => spend(name, -1000))
			);
			const page = await queue(context);
			await page.emulateMedia({ reducedMotion: 'reduce' });
			await page.setViewportSize({ width, height: 600 });
			const last = rows(page).last();
			const height = () => last.locator('.row').evaluate((e) => e.getBoundingClientRect().height);
			const before = await height();
			await last.getByLabel(say('column.category')).selectOption({ label: LONG });
			await expect(page.getByRole('textbox', { name: say('uncategorized.offer') })).toBeVisible();
			await page.waitForTimeout(1200);
			await page.evaluate(() => scrollTo(0, document.documentElement.scrollHeight));
			await still(page);
			if (width === 390)
				expect(await height(), 'the row takes a line more').toBeGreaterThan(before + 10);
			// As a press leaves it, without the tool's own scroll to the button.
			const press = page.getByRole('button', { name: say(key) });
			await press.evaluate((e) => e.focus({ preventScroll: true }));
			const drops = await watchDrops(page);
			await watchRows(page);
			await press.dispatchEvent('click');
			// It stood 168 px higher in a frame, the row with it (pass 45), after 24 px of its own (pass 46).
			await expectPageFollows(page, `${width}`, false);
			// The offer goes at once with the button in it: the focus moves on, never left on the page.
			expect(await drops(), `${width}`).toEqual([]);
			await sortAll(page);
			await page.close();
		}
	});
}

test('on a phone a notice never stays over the field that keeps the focus', async ({ context }) => {
	const names = 'ABCDEFGHIJKLMNOP'.split('').map((name) => `NOTE ${name}`);
	await fill(
		context,
		names.map((name) => spend(name, -1000))
	);
	const page = await queue(context);
	await page.setViewportSize({ width: 390, height: 600 });
	const select = rows(page).nth(3).getByLabel(say('column.category'));
	await select.focus();
	// Where a finger leaves it: under the bar, at the height the notice comes to.
	await select.evaluate((e) => scrollBy(0, e.getBoundingClientRect().top - 85));
	await still(page);
	expect(Math.abs(((await select.boundingBox())?.y ?? 0) - 85)).toBeLessThan(4);
	const refusal = { status: 500, code: 'unexpected', detail: 'no' };
	await page.route('**/api/v1/transactions/*', (route) =>
		route.fulfill({ status: 500, json: refusal })
	);
	// Chosen in the page itself: the tool scrolls a field into view before it selects, and a finger's place would be lost.
	await select.evaluate((e, id) => {
		(e as HTMLSelectElement).value = id;
		e.dispatchEvent(new Event('change', { bubbles: true }));
	}, food);
	const note = page.locator('.notices .note');
	await expect(note).toBeVisible();
	await expect(select).toBeFocused();
	// The select stood at 85–133 under a notice at 64–136, and six seconds later it still did (pass 45).
	await expect
		.poll(async () => {
			const [field, over] = [await select.boundingBox(), await note.boundingBox()];
			return (field?.y ?? 0) - (over?.y ?? 0) - (over?.height ?? 0);
		})
		.toBeGreaterThan(0);
	await page.unroute('**/api/v1/transactions/*');
	await sortAll(page);
	await page.close();
});

test('«Aplicar» that empties the queue falls in confetti, as the last choice by hand does', async ({
	context
}) => {
	const page = await answered(context, 'no-preference', 1280);
	await page.locator('.changes ~ .actions button').dispatchEvent('click');
	await expect(page.locator('#all-done')).toBeVisible();
	await expect(page.locator('body > div[aria-hidden="true"]')).toBeAttached();
	await page.close();
});
