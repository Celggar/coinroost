// The queue of transactions with no category, through the browser, against a real server (see run.sh): rows that
// filter and sort, a choice that stays with its Deshacer until the next one, and a choice turned into a rule on the
// spot. Its own user and its own data, made through the API.
import { expect, test, type Page } from '@playwright/test';
import { watchDrops } from './focus.ts';
import { expectFadedMoves, expectNoJump, expectPageFollows, watchRows } from './frames.ts';
import { say } from './messages.ts';
import {
	answered,
	choose,
	fill,
	filter,
	food,
	logIn,
	queue,
	row,
	rows,
	session,
	sortAll,
	spend
} from './queue.ts';
import { still } from './scroll.ts';

// A TOTP code is accepted once, so the run logs in once and the later tests reuse that session.
test.describe.configure({ mode: 'serial' });
test.use({ locale: 'es-PA' });
test.beforeAll(({ browser }) => logIn(browser));

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
	await expect(row(page, 'CAFE CENTRAL 0099').getByLabel(say('column.category'))).toBeFocused();

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
	await expect(page.getByRole('alert').first()).toHaveText(
		say('problem.status_5xx', { what: say('uncategorized.set_failed') })
	);
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
	const field = row(page, 'CAFE CENTRAL 0042').getByRole('textbox', {
		name: say('uncategorized.offer')
	});
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
	// The queue is empty by now: its rows come first, or it has no list to wait for.
	await context.addCookies(session.cookies);
	for (const text of ['COFFEE SHOP', 'BOOK STORE', 'HARDWARE STORE', 'BAKERY']) {
		const made = await context.request.post('/api/v1/transactions', { data: spend(text, -1000) });
		expect(made.ok()).toBe(true);
	}
	const page = await queue(context);
	await page.setViewportSize({ width: 1280, height: 720 });
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

test('the end card comes under the list after an empty filter', async ({ context }) => {
	await fill(context, [spend('LAST ONE', -500)]);
	const page = await queue(context);
	await page.setViewportSize({ width: 1280, height: 800 });
	await filter(page, 'income').check();
	await expect(page.getByText(say('uncategorized.none_here'))).toBeVisible();
	await filter(page, 'all').check();
	await choose(page, 'LAST ONE', 'Food');
	const end = page.locator('#all-done');
	await expect(end).toBeVisible();
	// The list's height stayed on the first list, gone with the empty filter: 0 px, and the card 178 px over the row.
	await expect
		.poll(async () => {
			const list = await page.locator('.queue').boundingBox();
			const card = await end.boundingBox();
			return card!.y - (list!.y + list!.height);
		})
		.toBeGreaterThanOrEqual(0);
	await page.close();
});

test('under reduced motion a row changes place only through a fade', async ({ context }) => {
	const names = ['ONE', 'TWO', 'THREE', 'FOUR', 'FIVE', 'SIX', 'SEVEN', 'EIGHT'];
	await fill(
		context,
		names.map((name) => spend(`ROW ${name}`, -1000))
	);
	const page = await queue(context);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	for (const width of [1280, 390]) {
		await page.setViewportSize({ width, height: 800 });
		await page.goto('/uncategorized');
		const first = page.locator('.queue select').first();
		await expect(first).toBeEnabled();
		await watchRows(page);
		await first.selectOption({ index: 1 });
		await expectFadedMoves(page, `${width}: the offer opens`);
		await watchRows(page);
		await page.locator('.queue > li:not(.sorted) select').first().selectOption({ index: 1 });
		await expectFadedMoves(page, `${width}: the next choice`);
		await watchRows(page);
		await page
			.locator('.queue > li.sorted')
			.getByRole('button', { name: say('uncategorized.undo') })
			.click();
		await expectFadedMoves(page, `${width}: undone`);
	}
	await sortAll(page);
	await page.close();
});

test('a leaving row keeps its line, at full width too', async ({ context }) => {
	await fill(context, [spend('STAYS ONE', -700), spend('STAYS TWO', -800), spend('STAYS 3', -900)]);
	const page = await queue(context);
	await page.setViewportSize({ width: 1280, height: 800 });
	await page.locator('.queue select').first().selectOption({ index: 1 });
	const sorted = page.locator('.queue > li.sorted');
	await expect(sorted).toHaveCount(1);
	const next = page.locator('.queue > li:not(.sorted) select').first();
	await expect(next).toBeFocused();
	// Its tint first: a row still changing colour stays in the flow as it leaves, and nothing would be tested.
	await sorted.evaluate((li) => Promise.all(li.getAnimations().map((tint) => tint.finished)));
	type Line = { out: boolean; broken: number[] };
	await sorted.evaluate((li) => {
		const w = window as unknown as Line;
		const parts = [...li.querySelectorAll('.date, .description, .amount, .control')];
		const place = () => {
			const row = li.getBoundingClientRect();
			return parts.flatMap((part) => {
				const box = part.getBoundingClientRect();
				return [box.left - row.left, box.top - row.top];
			});
		};
		const before = place();
		w.out = false;
		w.broken = [];
		const step = () => {
			if (!li.isConnected) return;
			w.out ||= getComputedStyle(li).position === 'absolute';
			w.broken.push(Math.max(...place().map((at, i) => Math.abs(at - before[i]))));
			requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	});
	await next.selectOption({ index: 1 });
	await expect(page.locator('.queue > li')).toHaveCount(2);
	const { out, broken } = await page.evaluate(() => {
		const w = window as unknown as Line;
		return { out: w.out, broken: w.broken };
	});
	expect(out).toBe(true);
	expect(Math.max(...broken)).toBeLessThan(2);
	await sortAll(page);
	await page.close();
});

type Swap = { frames: { rows: number; height: number; opacity: number }[] };

async function watchSwap(page: Page) {
	await page.evaluate(() => {
		const w = window as unknown as Swap;
		const box = document.querySelector<HTMLElement>('.follows')!;
		w.frames = [];
		const step = () => {
			w.frames.push({
				rows: [...document.querySelectorAll('.queue > li')].filter((li) => li.checkVisibility())
					.length,
				height: box.getBoundingClientRect().height,
				opacity: Number(getComputedStyle(box).opacity)
			});
			if (w.frames.length < 60) requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	});
}

test('a filter changes the list behind a fade, its room gliding', async ({ context }) => {
	const names = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'];
	await fill(context, [
		...names.map((name) => spend(`SPENT ${name}`, -1000)),
		spend('EARNED A', 2000),
		spend('EARNED B', 2000)
	]);
	const page = await queue(context);
	await page.setViewportSize({ width: 1280, height: 800 });
	for (const reducedMotion of ['no-preference', 'reduce'] as const) {
		await page.emulateMedia({ reducedMotion });
		await page.goto('/uncategorized');
		await expect(page.locator('.queue > li')).toHaveCount(10);
		for (const [name, count] of [
			['income', 2],
			['all', 10]
		] as const) {
			await watchSwap(page);
			await filter(page, name).check();
			await expect(page.locator('.queue > li')).toHaveCount(count);
			await page.waitForFunction(() => (window as unknown as Swap).frames.length >= 60);
			const frames = await page.evaluate(() => (window as unknown as Swap).frames);
			const way = `${name}, ${reducedMotion}`;
			// A height set after this frame's read shows in the next one: the fade it landed in is the lower of the two.
			const steps = frames.slice(1).map((frame, i) => ({
				...frame,
				opacity: Math.min(frame.opacity, frames[i].opacity),
				rowsChanged: frame.rows !== frames[i].rows,
				step: Math.abs(frame.height - frames[i].height)
			}));
			expect(steps.filter((frame) => frame.rowsChanged).length, way).toBeGreaterThan(0);
			expect(
				steps.filter((frame) => frame.rowsChanged && frame.opacity > 0.4),
				way
			).toEqual([]);
			const whole = Math.abs(frames[0].height - frames.at(-1)!.height);
			expect(whole, way).toBeGreaterThan(200);
			const biggest = steps.toSorted((a, b) => b.step - a.step)[0];
			// With motion the room glides; without, it lands behind the fade.
			if (reducedMotion === 'reduce') expect(biggest.opacity, way).toBeLessThan(0.4);
			else expect(biggest.step, way).toBeLessThan(whole / 2);
		}
	}
	// Quick keys: the list ends on the last one asked.
	await filter(page, 'all').focus();
	await page.keyboard.press('ArrowRight');
	await page.keyboard.press('ArrowRight');
	await expect(filter(page, 'income')).toBeChecked();
	await expect(page.locator('.queue > li')).toHaveCount(2);
	await sortAll(page);
	await page.close();
});

test('Deshacer under another filter keeps the row, its select and the focus', async ({
	context
}) => {
	await fill(context, [
		spend('SPENT ROW', -1000),
		spend('SPENT OTHER', -1100),
		spend('EARNED ROW', 2000)
	]);
	const page = await queue(context);
	await choose(page, 'SPENT ROW', 'Food');
	await expect(row(page, 'SPENT ROW')).toContainText('Food');
	await filter(page, 'income').check();
	// The filter's answer has come: the other spending has gone, the sorted row stays.
	await expect(rows(page)).toHaveCount(2);
	await row(page, 'SPENT ROW')
		.getByRole('button', { name: say('uncategorized.undo') })
		.click();
	await expect(row(page, 'SPENT ROW').getByLabel(say('column.category'))).toBeFocused();
	// The next choice lets it go.
	await choose(page, 'EARNED ROW', 'Pay');
	await expect(row(page, 'SPENT ROW')).toHaveCount(0);
	await sortAll(page);
	await page.close();
});

test('a key on a closed select opens it and chooses nothing', async ({ context }) => {
	await fill(context, [spend('KEY ONE', -1000), spend('KEY TWO', -1200)]);
	const page = await queue(context);
	const sent: string[] = [];
	page.on('request', (r) => {
		if (r.method() === 'PATCH') sent.push(r.url());
	});
	const select = row(page, 'KEY ONE').getByLabel(say('column.category'));
	const open = () => select.evaluate((e) => e.matches(':open'));
	for (const key of ['ArrowDown', 'f', 'End']) {
		await select.focus();
		await page.keyboard.press(key);
		await expect.poll(open, key).toBe(true);
		await page.keyboard.press('Escape');
		await expect.poll(open, key).toBe(false);
		await expect(select, key).toHaveValue('');
		expect(sent, key).toEqual([]);
	}
	// Open, the keys move inside it and Enter chooses.
	await select.focus();
	await page.keyboard.press('ArrowDown');
	await expect.poll(open).toBe(true);
	await page.keyboard.press('ArrowDown');
	expect(sent).toEqual([]);
	await page.keyboard.press('Enter');
	await expect.poll(() => sent.length).toBe(1);
	await sortAll(page);
	await page.close();
});

test('an answer in the offer keeps its whole focus ring, and Deshacer its whole button', async ({
	context
}) => {
	for (const [width, text] of [
		[1280, 'LONE MERCHANT'],
		[390, 'LONE SHOP']
	] as const) {
		await fill(context, [spend(text, -1000)]);
		const page = await queue(context);
		await page.setViewportSize({ width, height: 800 });
		await choose(page, text, 'Food');
		const send = row(page, text).getByRole('button', { name: say('uncategorized.make_rule') });
		await expect(send).toBeVisible();
		await send.press('Enter');
		const answer = page.locator('#changes-none');
		await expect(answer).toBeFocused();
		// The room around it once it has settled: it rises in as its box grows.
		const room = () =>
			answer.evaluate((p) => {
				const box = p.closest('section')!.getBoundingClientRect();
				const text = p.getBoundingClientRect();
				const style = getComputedStyle(p);
				const reach = parseFloat(style.outlineWidth) + parseFloat(style.outlineOffset);
				return reach > 0
					? Math.min(
							text.left - reach - box.left,
							box.right - text.right - reach,
							text.top - reach - box.top,
							box.bottom - text.bottom - reach
						)
					: -1;
			});
		await expect.poll(room, `${width}`).toBeGreaterThanOrEqual(0);
		const undo = row(page, text).getByRole('button', { name: say('uncategorized.undo') });
		// Its lower edge too: the answer's box reaches up, and must stop at the row's padding.
		const hit = () =>
			undo.evaluate((button) => {
				const box = button.getBoundingClientRect();
				return document.elementFromPoint(box.left + box.width / 2, box.bottom - 1) === button;
			});
		await expect.poll(hit, `${width}`).toBe(true);
		await page.close();
	}
});

for (const reducedMotion of ['no-preference', 'reduce'] as const) {
	test(`a rule from the offer answers and applies with no jump (${reducedMotion})`, async ({
		context
	}) => {
		// With motion a row may glide; with none it moves only behind a fade.
		const limit = reducedMotion === 'reduce' ? 2 : 100;
		for (const width of [1280, 390]) {
			const tag = `T${width}${reducedMotion[0]}`;
			await fill(context, [
				spend(`${tag} SHOP 01`, -1000),
				...['A', 'B', 'C', 'D', 'E'].map((name) => spend(`${tag} OTHER ${name}`, -1100)),
				spend(`${tag} SHOP 02`, -1200),
				spend(`${tag} SHOP 03`, -1300)
			]);
			const page = await queue(context);
			await page.emulateMedia({ reducedMotion });
			await page.setViewportSize({ width, height: 800 });
			await choose(page, `${tag} SHOP 01`, 'Food');
			const send = row(page, `${tag} SHOP 01`).getByRole('button', {
				name: say('uncategorized.make_rule')
			});
			await expect(send).toBeVisible();
			await page.waitForTimeout(500);

			// «Crear regla»: the form went first and the row stood 197, 73 and 290 px tall in three frames (pass 41).
			const drops = await watchDrops(page);
			await watchRows(page);
			await send.click();
			await expect(page.locator('#changes-some')).toBeFocused();
			await expectNoJump(page, `${width}: the answer`, limit);
			expect(await drops(), `${width}: the answer`).toEqual([]);

			// «Aplicar»: rows jumped 170 px at full opacity, and the page shrank 392 px in a frame (pass 41).
			const apply = page.getByRole('button', {
				name: new RegExp(say('changes.apply_other', { count: 2 }))
			});
			await watchRows(page);
			await apply.click();
			await expect(row(page, `${tag} SHOP`)).toHaveCount(0);
			await expectNoJump(page, `${width}: applied`, limit);
			expect(await drops(), `${width}: applied`).toEqual([]);
			await expect(page.locator('.queue select').first()).toBeFocused();
			await sortAll(page);
			await page.close();
		}
	});

	test(`a rule leaves the page where it stands when its answer shows there (${reducedMotion})`, async ({
		context
	}) => {
		await fill(
			context,
			Array.from({ length: 12 }, (_, i) => spend(`STAY ${i} ${reducedMotion}`, -1000))
		);
		const page = await queue(context);
		await page.emulateMedia({ reducedMotion });
		await page.setViewportSize({ width: 390, height: 844 });
		const third = rows(page).nth(2);
		await third.getByLabel(say('column.category')).selectOption({ label: 'Food' });
		const send = third.getByRole('button', { name: say('uncategorized.make_rule') });
		await expect(send).toBeVisible();
		// The choice's own landing comes first, once the offer has opened: scrolled before it, it undid this.
		await page.waitForTimeout(1200);
		await still(page);
		// The row near the window's top, as a person scrolled it there: the list's top is well above the window.
		await third.evaluate((e) => scrollBy(0, e.getBoundingClientRect().top - 66));
		await still(page);
		const before = await page.evaluate(() => scrollY);
		await send.dispatchEvent('click');
		await expect(page.locator('#changes-none')).toBeFocused();
		await page.waitForTimeout(800);
		// Only as far as a line cut at the top asks. Rows touching, it went up the whole list: 250 to 437 px (pass 42).
		const moved = (await page.evaluate(() => scrollY)) - before;
		expect(Math.abs(moved), `from ${before}`).toBeLessThan(60);
		await sortAll(page);
		await page.close();
	});

	test(`«Aplicar» that empties the queue takes the page up calmly (${reducedMotion})`, async ({
		context
	}) => {
		for (const width of [1280, 390]) {
			const page = await answered(context, reducedMotion, width);
			await watchRows(page);
			await page.locator('.changes ~ .actions button').dispatchEvent('click');
			await expect(page.locator('#all-done')).toBeVisible();
			await expectPageFollows(page, `${width}`, reducedMotion === 'no-preference');
			await page.close();
		}
	});

	test(`a filter from a scrolled page takes it to its new end calmly (${reducedMotion})`, async ({
		context
	}) => {
		// Half of each: the list shrinks past where the page stands, and the page has to go up with it.
		const names = 'ABCDEFGHIJ'.split('');
		await fill(
			context,
			names.flatMap((name) => [
				spend(`LONG ${name} ${reducedMotion}`, -1000),
				spend(`GOT ${name} ${reducedMotion}`, 2000)
			])
		);
		const page = await queue(context);
		await page.emulateMedia({ reducedMotion });
		await page.setViewportSize({ width: 1280, height: 800 });
		await page.evaluate(() => scrollTo(0, 120));
		await expect.poll(() => page.evaluate(() => scrollY)).toBe(120);
		await watchRows(page);
		// A real press where the pill stands: the tool's own click would scroll to it first.
		const pill = (await filter(page, 'spending').boundingBox())!;
		await page.mouse.click(pill.x + pill.width / 2, pill.y + pill.height / 2);
		await expect(page.locator('.queue > li')).toHaveCount(names.length);
		await expectPageFollows(page, reducedMotion, reducedMotion === 'no-preference');
		await sortAll(page);
		await page.close();
	});
}

test('under reduced motion the list goes up at the page end only through a fade', async ({
	context
}) => {
	for (const width of [1280, 390]) {
		const lines = 'ABCDEFGH'.split('').map((name) => spend(`LAST ${width} ${name}`, -1000));
		await fill(context, lines);
		const page = await queue(context);
		await page.emulateMedia({ reducedMotion: 'reduce' });
		await page.setViewportSize({ width, height: 600 });
		const last = rows(page).last();
		await last.getByLabel(say('column.category')).selectOption({ label: 'Food' });
		const only = last.getByRole('button', { name: say('uncategorized.only_this') });
		await expect(only).toBeVisible();
		await page.waitForTimeout(500);
		await page.evaluate(() => scrollTo(0, document.documentElement.scrollHeight));
		await still(page);
		await watchRows(page);
		// «Solo esta vez»: 168 px at 390 and 140 at 1280 in one frame, with the rows at full opacity (pass 42).
		await only.dispatchEvent('click');
		await expectPageFollows(page, `${width}: only this time`, false);
		await sortAll(page);
		await page.close();
	}
});

test('a filter with no row of its own says so beside the chosen row', async ({ context }) => {
	await fill(context, [spend('ONLY ONE', -1000), spend('EARNED TOO', 2000)]);
	const page = await queue(context);
	await filter(page, 'income').check();
	await expect(rows(page)).toHaveCount(1);
	await choose(page, 'EARNED TOO', 'Pay');
	await expect(page.getByText(say('uncategorized.none_here'))).toBeVisible();
	await expect(rows(page)).toHaveCount(1);
	await sortAll(page);
	await page.close();
});
