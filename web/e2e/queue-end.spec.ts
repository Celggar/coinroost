// The queue where the page ends and under a notice, through the browser, against a real server (see run.sh): its own
// user, as the categorizing spec has reached its size.
import { expect, test } from '@playwright/test';
import { expectPageFollows, watchRows } from './frames.ts';
import { say } from './messages.ts';
import { answered, fill, food, logIn, queue, rows, sortAll, spend } from './queue.ts';
import { still } from './scroll.ts';

// A TOTP code is accepted once, so the run logs in once and the later tests reuse that session.
test.describe.configure({ mode: 'serial' });
test.use({ locale: 'es-PA' });
test.beforeAll(({ browser }) => logIn(browser, 'QUEUE'));

test('under reduced motion «Deshacer» at the page end goes up only through a fade', async ({
	context
}) => {
	for (const width of [390, 1280]) {
		const names = 'ABCDEFGH'.split('').map((name) => `UNDO ${width} ${name}`);
		await fill(
			context,
			names.map((name) => spend(name, -1000))
		);
		const page = await queue(context);
		await page.emulateMedia({ reducedMotion: 'reduce' });
		await page.setViewportSize({ width, height: 600 });
		const last = rows(page).last();
		await last.getByLabel(say('column.category')).selectOption({ label: 'Food' });
		await expect(page.getByRole('textbox', { name: say('uncategorized.offer') })).toBeVisible();
		await page.waitForTimeout(1200);
		await page.evaluate(() => scrollTo(0, document.documentElement.scrollHeight));
		await still(page);
		await watchRows(page);
		await page.getByRole('button', { name: /^Deshacer/ }).dispatchEvent('click');
		// It stood 168 px higher in a frame, the row with it, at full opacity (pass 45).
		await expectPageFollows(page, `${width}`, false);
		await sortAll(page);
		await page.close();
	}
});

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
