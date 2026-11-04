// Rules moved and deleted through the browser, against a real server (see run.sh): the focus follows the rule and
// never falls to the page, and the page lands where the person can see what happened. Its own user.
import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import { watchDrops } from './focus.ts';
import { unfold } from './forms.ts';
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

test('on a phone, an error that opens in view leaves the page where it was', async ({
	context
}) => {
	const page = await loggedIn(context);
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto('/rules');
	const to = page.getByLabel(say('ruleform.max'), { exact: true });
	await unfold(page);
	await expect(to).toBeVisible();
	await page.getByLabel(say('ruleform.name'), { exact: true }).fill('Amounts reversed');
	await page.getByLabel(say('ruleform.text'), { exact: true }).fill('PAGO ');
	await page.getByLabel(say('ruleform.payee'), { exact: true }).selectOption('text');
	await page.getByLabel(say('ruleform.min'), { exact: true }).fill('50');
	await to.fill('5');
	// The field mid-screen with its label and the room for its error: the error opened there and the page still slid
	// 110 px to clear a line under the top bar (pass 22).
	await to.evaluate((e) => scrollBy(0, e.getBoundingClientRect().top - 374));
	const start = await still(page);
	type Moves = { ys: number[] };
	await page.evaluate(() => {
		const w = window as unknown as Moves;
		w.ys = [];
		const step = () => {
			w.ys.push(scrollY);
			if (w.ys.length < 60) requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	});
	await to.press('Enter');
	await expect(page.locator(`[id="${await to.getAttribute('id')}-error"]`)).toBeVisible();
	await page.waitForFunction(() => (window as unknown as Moves).ys.length >= 60);
	expect(new Set(await page.evaluate(() => (window as unknown as Moves).ys))).toEqual(
		new Set([start])
	);
	await expect(to).toBeFocused();
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

// After «Aplicar» «No cambiaría nada.» came in under the list, stood there beside it, and fell 355 px in one frame as the
// list went (pass 36). The answers come from the page's own routes here: what is judged is how one replaces the other.
for (const reducedMotion of ['no-preference', 'reduce'] as const) {
	test(`an applied answer takes the last one's place (${reducedMotion})`, async ({ context }) => {
		const page = await loggedIn(context);
		await page.emulateMedia({ reducedMotion });
		await page.setViewportSize({ width: 1280, height: 1300 });
		const row = (n: number) => ({
			transaction_id: `00000000-0000-4000-8000-00000000000${n}`,
			date: `2026-05-0${n}`,
			description: `Row ${n}`,
			amount_minor: -1000 * n,
			currency: 'USD',
			before: { category_id: null, payee_id: null },
			after: { category_id: null, payee_id: null }
		});
		let changes = [1, 2, 3, 4].map(row);
		await page.route('**/api/v1/rules/preview', (route) =>
			route.fulfill({ json: { tag: 'tag', changes } })
		);
		await page.route('**/api/v1/rules/apply', (route) => {
			changes = [];
			return route.fulfill({ json: { applied: 4 } });
		});
		await page.goto('/rules');
		await page.getByRole('button', { name: say('changes.check') }).click();
		const apply = page.getByRole('button', { name: say('changes.apply_other', { count: 4 }) });
		await expect(apply).toBeVisible();
		await still(page);

		// Every frame from here on: the new answer's place in the page and how much of it shows, and whether the last one
		// is still there.
		type Frame = { fresh: [number, number] | null; last: boolean };
		type Frames = { frames: Frame[] };
		await page.evaluate(() => {
			const w = window as unknown as Frames;
			w.frames = [];
			const step = () => {
				const fresh = document.querySelector('#changes-none');
				w.frames.push({
					fresh: fresh && [
						fresh.getBoundingClientRect().top + scrollY,
						Number(getComputedStyle(fresh).opacity)
					],
					last: document.querySelector('#changes-some') !== null
				});
				if (w.frames.length < 90) requestAnimationFrame(step);
			};
			requestAnimationFrame(step);
		});
		await apply.click();
		await page.waitForFunction(() => (window as unknown as Frames).frames.length >= 90);
		const frames = await page.evaluate(() => (window as unknown as Frames).frames);
		const arrived = frames.filter((frame) => frame.fresh);
		expect(arrived.length).toBeGreaterThan(3);
		expect(arrived.filter((frame) => frame.last && frame.fresh![1] > 0.01)).toEqual([]);
		const places = arrived.map((frame) => frame.fresh![0]);
		expect(Math.max(...places) - Math.min(...places)).toBeLessThan(16);
	});
}

// What «Ver cambios» answered was about the rules of that moment: with one added, moved or deleted it still said
// «No cambiaría nada.», while a new check said 3 transactions would change (pass 29).
test('an answer about the rules leaves by a fade when they change', async ({ context }) => {
	const page = await loggedIn(context);
	const [first] = await makeRules(page, ['Stale A', 'Stale B']);
	await page.goto('/rules');
	const answer = page.locator('#changes-none');
	type Frames = { seen: number[] };
	/** Asks, does what changes the rules, and reads the answer's opacity on every frame until it is gone. Inside the
	 * block that goes with it a local transition never ran: opacity 1, then nothing (pass 30). */
	const leaves = async (change: () => Promise<void>) => {
		await page.getByRole('button', { name: say('changes.check') }).click();
		await expect(answer).toHaveCSS('opacity', '1');
		await page.evaluate(() => {
			const w = window as unknown as Frames;
			const node = document.querySelector('#changes-none')!;
			w.seen = [];
			const step = () => {
				if (!node.isConnected) return;
				w.seen.push(Number(getComputedStyle(node).opacity));
				requestAnimationFrame(step);
			};
			requestAnimationFrame(step);
		});
		await change();
		await expect(answer).toHaveCount(0);
		const seen = await page.evaluate(() => (window as unknown as Frames).seen);
		expect(Math.min(...seen)).toBeLessThan(0.9);
	};

	await leaves(() => page.locator(`[data-move="${first}-down"]`).click());
	await leaves(async () => {
		await page
			.getByRole('button', { name: say('rules.delete_named', { name: 'Stale A' }) })
			.click();
		await page.getByRole('button', { name: say('rules.delete_it') }).click();
	});
	await leaves(async () => {
		await page.locator('#new-rule').click();
		await unfold(page);
		await page.getByLabel(say('ruleform.name'), { exact: true }).fill('Stale C');
		await page.getByLabel(say('ruleform.text'), { exact: true }).fill('STALE C');
		await page.getByLabel(say('ruleform.payee'), { exact: true }).selectOption('text');
		await page.getByRole('button', { name: say('ruleform.add') }).click();
		await expect(page.getByRole('heading', { name: 'Stale C' })).toBeVisible();
	});

	for (const { id } of (await (await page.request.get('/api/v1/rules')).json()).items) {
		expect((await page.request.delete(`/api/v1/rules/${id}`)).ok()).toBe(true);
	}
});

// With reduced motion a lone rule's list already held its height until it had faded, then went at once, as it should.
for (const [names, gone, reducedMotion] of [
	[['Foot A', 'Foot B'], 'Foot B', 'no-preference'],
	[['Foot A', 'Foot B'], 'Foot B', 'reduce'],
	[['Alone'], 'Alone', 'no-preference']
] as const) {
	test(`what follows the list waits while «${gone}» fades (${reducedMotion})`, async ({
		context
	}) => {
		const page = await loggedIn(context);
		await page.emulateMedia({ reducedMotion });
		const made = await makeRules(page, [...names]);
		await page.goto('/rules');
		const ask = page.getByRole('button', { name: say('rules.delete_named', { name: gone }) });
		await expect(ask).toBeVisible();
		await ask.click();
		const yes = page.getByRole('button', { name: say('rules.delete_it') });
		await still(page);
		// Every frame: where the block after the form stands in the page, the page's own scroll left out, and how tall
		// the first rule stands. A row that leaves goes out of the flow at once, and what follows the list went up 238 px
		// in one frame, under the rule still fading (pass 25); held by the list's box, the rules left stretched into its
		// room, over the one leaving (pass 26).
		type Frames = { moved: number[]; tops: number[]; heights: number[] };
		await page.evaluate(
			(id) => {
				const w = window as unknown as Frames;
				const leaving = document.querySelector(`[data-rule="${id}"]`)!;
				const after = document.querySelector('section.follows')!;
				const top = () => after.getBoundingClientRect().top + scrollY;
				const start = top();
				w.moved = [];
				w.tops = [];
				w.heights = [];
				const first = document.querySelector('ol.rules > li')!;
				const step = () => {
					w.tops.push(top());
					if (first !== leaving) w.heights.push(first.getBoundingClientRect().height);
					if (leaving.isConnected && Number(getComputedStyle(leaving).opacity) > 0.02) {
						w.moved.push(top() - start);
					}
					if (w.tops.length < 90) requestAnimationFrame(step);
				};
				requestAnimationFrame(step);
			},
			made[made.length - 1]
		);
		await yes.click();
		await page.waitForFunction(() => (window as unknown as Frames).tops.length >= 90);
		const { moved, tops, heights } = await page.evaluate(() => {
			const w = window as unknown as Frames;
			return { moved: w.moved, tops: w.tops, heights: w.heights };
		});
		expect(new Set(heights.map(Math.round)).size, heights.join(' ')).toBeLessThanOrEqual(1);
		expect(moved.length).toBeGreaterThan(3);
		expect(moved.filter((offset) => Math.abs(offset) > 0.5)).toEqual([]);
		// Up as the list shrinks; with the last rule gone, down, as the form that takes its place opens.
		const travel = Math.abs(tops[0] - tops[tops.length - 1]);
		expect(travel).toBeGreaterThan(100);
		if (reducedMotion === 'no-preference') {
			const steps = tops.slice(1).map((top, i) => Math.abs(top - tops[i]));
			expect(Math.max(...steps), tops.map(Math.round).join(' ')).toBeLessThan(travel / 2);
		}
		for (const id of made.slice(0, -1)) {
			expect((await page.request.delete(`/api/v1/rules/${id}`)).ok()).toBe(true);
		}
		await page.emulateMedia({ reducedMotion: null });
	});
}

for (const reducedMotion of ['no-preference', 'reduce'] as const) {
	test(`a notice waits for the one it replaces: never two at once (${reducedMotion})`, async ({
		context
	}) => {
		const page = await loggedIn(context);
		await page.emulateMedia({ reducedMotion });
		await page.route('**/api/v1/rules/preview', (route) => route.fulfill({ status: 503 }));
		await page.goto('/rules');
		const check = page.getByRole('button', { name: say('changes.open') });
		await check.click();
		await expect(page.locator('.note')).toBeVisible();
		await still(page, 500);
		// Every frame: how many notices show. The second came in over the first, the same words 4 px off (pass 25).
		type Frames = { shown: number[] };
		await page.evaluate(() => {
			const w = window as unknown as Frames;
			w.shown = [];
			const step = () => {
				const notes = [...document.querySelectorAll('.note')];
				w.shown.push(notes.filter((n) => Number(getComputedStyle(n).opacity) > 0.02).length);
				if (w.shown.length < 60) requestAnimationFrame(step);
			};
			requestAnimationFrame(step);
		});
		await check.click();
		await page.waitForFunction(() => (window as unknown as Frames).shown.length >= 60);
		const shown = await page.evaluate(() => (window as unknown as Frames).shown);
		expect(Math.max(...shown)).toBe(1);
		expect(shown[shown.length - 1]).toBe(1);
		await page.emulateMedia({ reducedMotion: null });
	});
}

test('«Nueva regla» opens the form in place, what few rules need folded', async ({ context }) => {
	const page = await loggedIn(context);
	const made = await makeRules(page, ['Open A']);
	await page.goto('/rules');
	const add = page.locator('#new-rule');
	const form = page.getByRole('form', { name: say('ruleform.title') });
	await expect(add).toBeVisible();
	await expect(form).toHaveCount(0);
	await add.click();
	await expect(form.getByLabel(say('ruleform.name'))).toBeFocused();
	await expect(form.getByLabel(say('ruleform.min'))).toBeHidden();
	await form.getByRole('button', { name: say('ruleform.cancel') }).click();
	await expect(form).toHaveCount(0);
	await expect(add).toBeFocused();
	expect((await page.request.delete(`/api/v1/rules/${made[0]}`)).ok()).toBe(true);
});

test('on a phone a new rule is a full-screen dialog, sent from its top', async ({ context }) => {
	const page = await loggedIn(context);
	await page.setViewportSize({ width: 390, height: 844 });
	const made = await makeRules(page, ['Sheet A']);
	await page.goto('/rules');
	const add = page.locator('#new-rule');
	await add.click();
	const sheet = page.getByRole('dialog', { name: say('ruleform.title') });
	const name = sheet.getByLabel(say('ruleform.name'));
	await expect(name).toBeFocused();
	await sheet.evaluate((e) => Promise.all(e.getAnimations().map((a) => a.finished.catch(() => 0))));
	expect(await sheet.boundingBox()).toEqual({ x: 0, y: 0, width: 390, height: 844 });
	// Esc leaves it, and the focus goes back to the button: modal to its last frame, the page behind took none.
	await page.keyboard.press('Escape');
	await expect(add).toBeFocused();
	await expect(sheet).toHaveCount(0);
	// Sent, the new rule has the focus at once, the dialog still sliding away over it.
	await add.click();
	await name.fill('Sheet B');
	await sheet.getByLabel(say('ruleform.text')).fill('SHEET B');
	await unfold(page);
	await sheet.getByLabel(say('ruleform.payee')).selectOption('text');
	const drops = await watchDrops(page);
	await sheet.getByRole('button', { name: say('ruleform.save') }).click();
	const row = page.locator('ol.rules > li', { hasText: 'Sheet B' });
	await expect(row).toBeFocused();
	await expect(sheet).toHaveCount(0);
	expect(await drops()).toEqual([]);
	made.push((await row.getAttribute('data-rule'))!);
	for (const id of made) expect((await page.request.delete(`/api/v1/rules/${id}`)).ok()).toBe(true);
	await page.setViewportSize({ width: 1280, height: 720 });
});
