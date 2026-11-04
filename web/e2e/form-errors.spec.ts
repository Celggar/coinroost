// Form errors in Spanish, as a person meets them: nothing is left to the browser's own bubble (which speaks the
// browser's language, outside the page), each error sits under its field in the page's language with the field
// marked and focused, and no English detail from the API reaches the page; and where the page lands once a form is
// sent. Its own user (see run.sh).
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { watchDrops } from './focus.ts';
import { say } from './messages.ts';
import { makeRules } from './rules.ts';
import { still } from './scroll.ts';
import { totp } from './totp.ts';

test.use({ locale: 'es-PA' });
test.describe.configure({ mode: 'serial' });

// What the API says in English; none of it may show in the Spanish page.
const ENGLISH =
	/it already exists|cannot be read|choose two different|sum to zero|unreadable date|no amount that/i;

let page: Page;

test.beforeAll(async ({ browser }) => {
	const secret = process.env.E2E_FORMS_SECRET ?? '';
	expect(secret, 'run through e2e/run.sh').not.toBe('');
	page = await browser.newPage({ locale: 'es-PA' });
	await page.goto('/accounts');
	await page.getByLabel(say('login.email')).fill(process.env.E2E_FORMS_EMAIL ?? '');
	await page.getByLabel(say('login.password')).fill(process.env.E2E_FORMS_PASSWORD ?? '');
	await page.getByLabel(say('login.code')).fill(totp(secret));
	await page.getByRole('button', { name: say('login.title') }).click();
	await expect(page.getByRole('heading', { name: say('accounts.title') })).toBeVisible();
});

test.afterAll(() => page.close());

/** The field's error is the given text, under it and tied to it, and the field is marked invalid. */
async function fieldSays(label: string, text: string) {
	const field = page.getByLabel(label, { exact: true });
	await expect(field).toHaveAttribute('aria-invalid', 'true');
	const ids = (await field.getAttribute('aria-describedby')) ?? '';
	const error = ids.split(' ').find((id) => id.endsWith('-error'));
	expect(error, `${label} names its error`).toBeTruthy();
	await expect(page.locator(`[id="${error}"]`)).toHaveText(text);
	await expect(page.locator(`[id="${error}"]`)).toBeVisible();
	return field;
}

test('an empty form says what is missing under each field, in Spanish', async () => {
	const form = page.locator('form').first();
	expect(await form.evaluate((f: HTMLFormElement) => f.noValidate)).toBe(true);
	await page.getByRole('button', { name: say('accounts.create') }).click();
	const name = await fieldSays(say('accounts.name'), say('form.required'));
	await expect(name).toBeFocused();
	// Typing clears that field's message.
	await name.fill('Corriente');
	await expect(name).not.toHaveAttribute('aria-invalid', 'true');
});

test('a key already taken is marked on «Clave», not as a loose English line', async () => {
	await page.getByLabel(say('accounts.name'), { exact: true }).fill('Corriente');
	await page.getByLabel(say('accounts.bank_file')).selectOption({ value: 'bac-account' });
	await page.getByLabel(say('accounts.key')).fill('corriente');
	await page.getByRole('button', { name: say('accounts.create') }).click();
	await expect(page.getByRole('status')).toHaveText(say('accounts.created', { name: 'Corriente' }));

	await page.getByLabel(say('accounts.name'), { exact: true }).fill('Otra');
	await page.getByLabel(say('accounts.key')).fill('corriente');
	await page.getByRole('button', { name: say('accounts.create') }).click();
	const key = await fieldSays(say('accounts.key'), say('problem.already_exists.namespace'));
	await expect(key).toBeFocused();
	await expect(page.locator('main')).not.toContainText(ENGLISH);
});

test('every wrong field is marked at once, in the words the API would use', async () => {
	// The API answers one field at a time: three wrong fields took three sends (pass 14). The form checks the API's
	// currency and key rules before sending.
	await page.getByLabel(say('accounts.name'), { exact: true }).fill('');
	await page.getByLabel(say('accounts.currency'), { exact: true }).fill('us');
	await page.getByLabel(say('accounts.key'), { exact: true }).fill('Bad Key!');
	await page.getByRole('button', { name: say('accounts.create') }).click();
	await fieldSays(say('accounts.name'), say('form.required'));
	await fieldSays(say('accounts.currency'), say('problem.currency_unknown'));
	await fieldSays(say('accounts.key'), say('problem.namespace_format'));
});

test('an amount that is not money is marked on its field before anything is sent', async () => {
	await page.goto('/rules');
	const form = page.getByRole('form', { name: say('ruleform.title') });
	await form.getByLabel(say('ruleform.name')).fill('Prueba');
	await form.getByLabel(say('ruleform.min')).fill('abc');
	await form.getByRole('button', { name: say('ruleform.add') }).click();
	const min = await fieldSays(say('ruleform.min'), say('form.amount'));
	await expect(min).toBeFocused();
	// A signed amount is money: it is told what the API would say, that a rule's amounts carry no sign.
	await min.fill('-5');
	await form.getByRole('button', { name: say('ruleform.add') }).click();
	await fieldSays(say('ruleform.min'), say('problem.rule_negative_amount'));
});

test('rows that cannot be read say why in Spanish', async () => {
	const fixture = fileURLToPath(
		new URL('../../spec/fixtures/bac-account/input.csv', import.meta.url)
	);
	// The statement already has a date written another way (row 9); one row also loses its amounts.
	const text = readFileSync(fixture, 'latin1').replace(
		'YAPPY A JOHN DOE          , 20.00, 0.00,',
		'YAPPY A JOHN DOE          , , ,'
	);
	await page.goto('/import');
	await page.locator('input[type=file]').setInputFiles({
		name: 'estado.csv',
		mimeType: 'text/csv',
		buffer: Buffer.from(text, 'latin1')
	});
	const card = page.getByRole('article', { name: 'estado.csv' });
	const account = card.getByLabel(say('file.account'));
	if (await account.isVisible()) await account.selectOption({ label: 'Corriente' });
	await expect(card.getByText(say('row.unreadable_date', { value: '2026/05/20' }))).toBeVisible();
	await expect(card.getByText(say('row.unreadable_amount'))).toBeVisible();
	await expect(page.locator('main')).not.toContainText(ENGLISH);
});

test('an account that cannot read the file shrinks its card without moving the page in one frame', async () => {
	const response = await page.request.post('/api/v1/accounts', {
		data: {
			name: 'Tarjeta',
			kind: 'credit_card',
			currency: 'USD',
			namespace: 'tarjeta',
			profile: 'bac-card'
		}
	});
	expect(response.status()).toBe(201);
	const fixture = fileURLToPath(new URL('../../spec/fixtures/bac-card/input.csv', import.meta.url));
	await page.goto('/import');
	await page
		.locator('input[type=file]')
		.setInputFiles({ name: 'tarjeta.csv', mimeType: 'text/csv', buffer: readFileSync(fixture) });
	const card = page.getByRole('article', { name: 'tarjeta.csv' });
	await expect(card.getByText(say('file.rows_read'))).toBeVisible();
	// A short window, and the card grown to its full height before the page goes to its end.
	await page.setViewportSize({ width: 1280, height: 600 });
	let height = 0;
	await expect
		.poll(async () => height === (height = await page.evaluate(() => document.body.scrollHeight)), {
			intervals: [300]
		})
		.toBe(true);
	await page.evaluate(() => scrollTo(0, document.documentElement.scrollHeight));
	await still(page);
	// Every frame from the choice on, at the page's end: the figures left, the card shrank at once and the browser
	// pulled the page back 98 px in that same frame (pass 15).
	type Frames = { frames: number[] };
	await page.evaluate(() => {
		const w = window as unknown as Frames;
		w.frames = [];
		const step = () => {
			w.frames.push(scrollY);
			if (w.frames.length < 150) requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	});
	await card.getByLabel(say('file.account')).selectOption({ label: 'Corriente' });
	await page.waitForFunction(() => (window as unknown as Frames).frames.length >= 150);
	const frames = await page.evaluate(() => (window as unknown as Frames).frames);
	await expect(card.getByText(say('file.rows_read'))).toHaveCount(0);
	const travel = frames[0] - frames[frames.length - 1];
	const steps = frames.slice(1).map((y, i) => Math.abs(y - frames[i]));
	// The page did move, so the case is the one that jumped; it glides, no frame taking half the way.
	expect(travel, `the page from ${frames[0]}`).toBeGreaterThan(40);
	expect(Math.max(...steps), `${travel} px in all`).toBeLessThan(travel / 2);
	await page.setViewportSize({ width: 1280, height: 720 });
});

test('on a phone, a new account shows whole above its form', async () => {
	await page.setViewportSize({ width: 390, height: 844 });
	for (const [name, kind] of [
		['Ahorro', 'savings'],
		['Efectivo', 'cash']
	]) {
		const response = await page.request.post('/api/v1/accounts', {
			data: { name, kind, currency: 'USD', namespace: name.toLowerCase() }
		});
		expect(response.status()).toBe(201);
	}
	await page.goto('/accounts');
	const name = page.getByLabel(say('accounts.name'), { exact: true });
	await name.fill('Zeta');
	await page.getByRole('button', { name: say('accounts.create') }).click();
	await expect(name).toBeFocused();
	const row = page.locator('tbody tr', { hasText: 'Zeta' });
	await row.evaluate((e) => Promise.all(e.getAnimations().map((a) => a.finished.catch(() => 0))));
	await still(page, 500);
	// Shown from the form's title, the new account, last in the list, stood above the screen (pass 15): it shows
	// whole under the top bar, and the field under it, above the bottom one.
	const [bar, top, bottom, field, end] = await row.evaluate((e) => {
		const rect = e.getBoundingClientRect();
		const input = document.querySelector('input[name="name"]')!.getBoundingClientRect();
		return [
			document.querySelector('header.appbar')!.getBoundingClientRect().bottom,
			rect.top,
			rect.bottom,
			input.bottom,
			document.querySelector('nav.bar')!.getBoundingClientRect().top
		];
	});
	expect(top).toBeGreaterThanOrEqual(bar);
	expect(bottom).toBeLessThan(field);
	expect(field).toBeLessThanOrEqual(end);
	await page.setViewportSize({ width: 1280, height: 720 });
});

test('on a phone, a new account sorted far above its form shows alone, with the focus', async () => {
	await page.setViewportSize({ width: 390, height: 844 });
	for (const name of ['Moneda', 'Viajes']) {
		const response = await page.request.post('/api/v1/accounts', {
			data: { name, kind: 'checking', currency: 'USD', namespace: name.toLowerCase() }
		});
		expect(response.status()).toBe(201);
	}
	await page.goto('/accounts');
	await page.getByLabel(say('accounts.name'), { exact: true }).fill('Aaa');
	// Down to the button first, as a person does: the field they filled scrolls away, and the press takes its focus.
	const create = page.getByRole('button', { name: say('accounts.create') });
	await create.scrollIntoViewIfNeeded();
	await create.focus();
	await still(page);
	// Every frame from the press: the name field never holds the focus under the top bar, and the focus never drops
	// to the page. Given the focus at once, the field sat there, scrolled past, while the page settled (pass 16); the
	// button disabled while it worked left the focus on the page until the landing (pass 17).
	await page.evaluate(() => {
		const bar = document.querySelector('header.appbar')!;
		const field = document.querySelector('input[name="name"]')!;
		const frames: string[] = [];
		(window as unknown as { focus: string[] }).focus = frames;
		const sample = () => {
			const active = document.activeElement;
			const under =
				active === field && field.getBoundingClientRect().top < bar.getBoundingClientRect().bottom;
			frames.push(under ? 'field under the bar' : active === document.body ? 'page' : '');
			if (frames.length < 150) requestAnimationFrame(sample);
		};
		requestAnimationFrame(sample);
	});
	await create.click();
	const row = page.locator('tbody tr', { hasText: 'Aaa' });
	// Left on the form, the new account, first in the list, stood 1342 px above the screen (pass 16).
	await expect(row).toBeFocused();
	await still(page, 500);
	const [bar, top, bottom, end] = await row.evaluate((e) => {
		const rect = e.getBoundingClientRect();
		return [
			document.querySelector('header.appbar')!.getBoundingClientRect().bottom,
			rect.top,
			rect.bottom,
			document.querySelector('nav.bar')!.getBoundingClientRect().top
		];
	});
	expect(top).toBeGreaterThanOrEqual(bar);
	expect(bottom).toBeLessThanOrEqual(end);
	await expect
		.poll(() => page.evaluate(() => (window as unknown as { focus: string[] }).focus.length))
		.toBe(150);
	const focus = await page.evaluate(() => (window as unknown as { focus: string[] }).focus);
	expect(focus.filter(Boolean)).toEqual([]);
	// One Tab and the form is ready for the next.
	await page.keyboard.press('Tab');
	await expect(page.getByLabel(say('accounts.name'), { exact: true })).toBeFocused();
	await page.setViewportSize({ width: 1280, height: 720 });
});

test('a file removed at the page end leaves the page gliding, never pulled back as it shortens', async () => {
	const fixture = fileURLToPath(
		new URL('../../spec/fixtures/bac-account/input.csv', import.meta.url)
	);
	await page.setViewportSize({ width: 1280, height: 600 });
	await page.goto('/import');
	await page.locator('input[type=file]').setInputFiles([
		{ name: 'corriente.csv', mimeType: 'text/csv', buffer: readFileSync(fixture) },
		{ name: 'notas.txt', mimeType: 'text/plain', buffer: Buffer.from('just, some\nwords') }
	]);
	await expect(page.getByText(say('file.rows_read'))).toBeVisible();
	await expect(page.getByText(say('file.not_found'))).toBeVisible();
	await page.evaluate(() => scrollTo(0, document.documentElement.scrollHeight));
	await still(page);
	// Every frame from the click on: the card left at the page's end, the page shrank and the browser pulled it back
	// 218 px in that same frame, before it glided up to the files' zone (pass 15).
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
	await page.getByRole('button', { name: say('file.remove_named', { name: 'notas.txt' }) }).click();
	await page.waitForFunction(() => (window as unknown as Frames).frames.length >= 150);
	const frames = await page.evaluate(() => (window as unknown as Frames).frames);
	const pulled = frames
		.slice(1)
		.filter(([y, height], i) => height < frames[i][1] && y !== frames[i][0]);
	expect(frames[0][0] - frames[frames.length - 1][0]).toBeGreaterThan(40);
	expect(pulled).toEqual([]);
	await expect(page.locator('.drop input')).toBeFocused();
	await page.setViewportSize({ width: 1280, height: 720 });
});

test('a pair confirmed at the page end leaves the page gliding as the pairing form goes', async () => {
	const post = async (url: string, data: object) => {
		const response = await page.request.post(url, { data });
		expect(response.status(), url).toBe(201);
		return response.json();
	};
	const accounts: { id: string; name: string }[] = (
		await (await page.request.get('/api/v1/accounts')).json()
	).items;
	const own = await post('/api/v1/categories', { name: 'Propias', kind: 'transfer' });
	for (const [name, amount] of [
		['Corriente', -50000],
		['Ahorro', 50000]
	] as const) {
		await post('/api/v1/transactions', {
			date: '2026-05-15',
			description: 'BETWEEN ACCOUNTS',
			category_id: own.id,
			legs: [
				{
					account_id: accounts.find((a) => a.name === name)!.id,
					amount_minor: amount,
					statement_text: 'BETWEEN ACCOUNTS'
				}
			]
		});
	}
	await page.setViewportSize({ width: 1280, height: 800 });
	await page.goto('/transfers');
	const confirm = page.getByRole('button', { name: say('waiting.pair_one') });
	await expect(page.getByRole('form', { name: say('unmatched.by_hand') })).toBeVisible();
	await page.evaluate(() => scrollTo(0, document.documentElement.scrollHeight));
	await still(page);
	await expect(confirm).toBeInViewport();
	// Every frame from the click on: the pairing form left with the pair, its card shrank at once and the browser
	// pulled the page back 232 px in that frame (pass 16 sweep).
	type Frames = { frames: number[] };
	await page.evaluate(() => {
		const w = window as unknown as Frames;
		w.frames = [];
		const step = () => {
			w.frames.push(scrollY);
			if (w.frames.length < 150) requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	});
	await confirm.click({ force: true });
	await page.waitForFunction(() => (window as unknown as Frames).frames.length >= 150);
	const frames = await page.evaluate(() => (window as unknown as Frames).frames);
	await expect(page.getByRole('form', { name: say('unmatched.by_hand') })).toHaveCount(0);
	const travel = frames[0] - frames[frames.length - 1];
	const steps = frames.slice(1).map((y, i) => Math.abs(y - frames[i]));
	expect(travel, `the page from ${frames[0]}`).toBeGreaterThan(40);
	expect(Math.max(...steps), `${travel} px in all`).toBeLessThan(travel / 2);
	await page.setViewportSize({ width: 1280, height: 720 });
});

test('a split pair takes its place without the old line drawn over it', async () => {
	await page.setViewportSize({ width: 1280, height: 800 });
	type Frames = { frames: number[] };
	for (const reducedMotion of ['no-preference', 'reduce'] as const) {
		await page.emulateMedia({ reducedMotion });
		await page.goto('/transfers');
		await page
			.getByRole('button', { name: /^Separar/ })
			.first()
			.click();
		// Every frame: how many of «Parejas por confirmar»'s blocks show in its one cell. «Nada por confirmar» was
		// drawn over the new pair while it faded (pass 16).
		await page.evaluate(() => {
			const w = window as unknown as Frames;
			w.frames = [];
			const cell = document.querySelector('[aria-labelledby="waiting-title"] .in-place')!;
			const step = () => {
				w.frames.push(
					[...cell.children].filter((e) => Number(getComputedStyle(e).opacity) > 0.02).length
				);
				if (w.frames.length < 90) requestAnimationFrame(step);
			};
			requestAnimationFrame(step);
		});
		const drops = await watchDrops(page);
		await page.getByRole('button', { name: say('made.split_it') }).click();
		await page.waitForFunction(() => (window as unknown as Frames).frames.length >= 90);
		const frames = await page.evaluate(() => (window as unknown as Frames).frames);
		expect(
			frames.filter((shown) => shown > 1),
			reducedMotion
		).toEqual([]);
		await page.getByRole('button', { name: say('waiting.pair_one') }).click();
		await expect(page.getByRole('button', { name: /^Separar/ })).toHaveCount(1);
		await expect(page.locator('#made-title')).toBeFocused();
		expect(await drops()).toEqual([]);
	}
	await page.emulateMedia({ reducedMotion: null });
	await page.setViewportSize({ width: 1280, height: 720 });
});

test('a rule added above its form opens its room while the form stays still on the screen', async () => {
	await page.setViewportSize({ width: 1280, height: 800 });
	await makeRules(page, ['Slide A', 'Slide B']);
	await page.goto('/rules');
	const form = page.getByRole('form', { name: say('ruleform.title') });
	await form.getByLabel(say('ruleform.name')).fill('Slide C');
	await form.getByLabel(say('ruleform.text')).fill('SLIDE C');
	await form.getByLabel(say('ruleform.category')).selectOption({ index: 1 });
	// The last rule and the form in view, the press already on the button.
	await form.evaluate((e) => scrollBy(0, e.getBoundingClientRect().top - 260));
	const add = form.getByRole('button', { name: say('ruleform.add') });
	await add.focus();
	await still(page);
	// Every frame: the form's place in the page (the page's own scroll left out), its place on the screen while the
	// rule grows, and where the focus is. The form jumped 238 px in one frame as the new rule came in above it at its
	// whole height; the focus dropped to the page while the button was disabled, then went to the name field before
	// it showed (pass 17).
	type Frames = { tops: number[]; growing: number[]; focus: string[] };
	await form.evaluate((e) => {
		const w = window as unknown as Frames;
		w.tops = [];
		w.growing = [];
		w.focus = [];
		const step = () => {
			w.tops.push(e.getBoundingClientRect().top + scrollY);
			if (document.querySelector('[data-growing]')) w.growing.push(e.getBoundingClientRect().top);
			const active = document.activeElement;
			const box = active?.getBoundingClientRect();
			w.focus.push(
				active === document.body
					? 'page'
					: box && (box.bottom <= 0 || box.top >= innerHeight)
						? `${active!.tagName} off the screen`
						: ''
			);
			if (w.tops.length < 90) requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	});
	await add.click();
	await page.waitForFunction(() => (window as unknown as Frames).tops.length >= 90);
	const { tops, growing, focus } = await page.evaluate(() => {
		const w = window as unknown as Frames;
		return { tops: w.tops, growing: w.growing, focus: w.focus };
	});
	const moved = tops[tops.length - 1] - tops[0];
	const steps = tops.slice(1).map((top, i) => Math.abs(top - tops[i]));
	expect(moved).toBeGreaterThan(100);
	expect(Math.max(...steps), `${moved} px in all`).toBeLessThan(moved / 2);
	// The room opens over several frames, and the page follows it: the form, in use, holds its place on the screen.
	expect(growing.length).toBeGreaterThan(5);
	expect(Math.max(...growing) - Math.min(...growing), growing.join(' ')).toBeLessThanOrEqual(2);
	expect(focus.filter(Boolean)).toEqual([]);
	await expect(form.getByLabel(say('ruleform.name'))).toBeFocused();
	await page.setViewportSize({ width: 1280, height: 720 });
});

test('the transfers summary grows and shrinks with its figures, never in one frame', async () => {
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto('/transfers');
	// Loaded: its figures, or the line that says there are none.
	const summary = page.locator('[aria-labelledby="summary-title"]');
	await expect(
		summary
			.locator('dl.figures')
			.or(summary.getByText(say('summary.nothing')))
			.first()
	).toBeVisible();
	await still(page);
	type Frames = { heights: number[] };
	// The summary card's height every frame, while `act` runs and the page settles.
	const heights = async (act: () => Promise<void>) => {
		await page.evaluate(() => {
			const w = window as unknown as Frames;
			const card = document.querySelector('[aria-labelledby="summary-title"]')!;
			w.heights = [];
			const step = () => {
				w.heights.push(card.getBoundingClientRect().height);
				if (w.heights.length < 120) requestAnimationFrame(step);
			};
			requestAnimationFrame(step);
		});
		await act();
		await page.waitForFunction(() => (window as unknown as Frames).heights.length >= 120);
		return page.evaluate(() => (window as unknown as Frames).heights);
	};
	const split = await heights(async () => {
		await page
			.getByRole('button', { name: /^Separar/ })
			.first()
			.click();
		await page.getByRole('button', { name: say('made.split_it') }).click();
	});
	const paired = await heights(() =>
		page.getByRole('button', { name: say('waiting.pair_one') }).click()
	);
	// Its figures came and went with the pair: they took 173 px in one frame, and every card under them jumped (pass 16).
	for (const [what, frames] of [
		['split', split],
		['paired', paired]
	] as const) {
		const change = Math.abs(frames[frames.length - 1] - frames[0]);
		const steps = frames.slice(1).map((h, i) => Math.abs(h - frames[i]));
		expect(change, what).toBeGreaterThan(40);
		expect(Math.max(...steps), `${what}: ${change} px in all`).toBeLessThan(change / 2);
	}
	await page.setViewportSize({ width: 1280, height: 720 });
});

// Last: it imports into «Corriente», which the tests above read without imports.
test('importing keeps the rows table still while each status changes', async () => {
	const fixture = fileURLToPath(
		new URL('../../spec/fixtures/bac-account/input.csv', import.meta.url)
	);
	await page.goto('/import');
	await page
		.locator('input[type=file]')
		.setInputFiles({ name: 'mayo.csv', mimeType: 'text/csv', buffer: readFileSync(fixture) });
	const card = page.getByRole('article', { name: 'mayo.csv' });
	await card.locator('summary').click();
	const amounts = card.locator('tbody td.number');
	await expect(amounts.first()).toBeVisible();
	const edges = () =>
		amounts.evaluateAll((cells) => cells.map((c) => c.getBoundingClientRect().right));
	const before = await edges();
	await card.getByRole('button', { name: /^Importar/ }).click();
	await expect(card.getByText(say('file.imported'))).toBeVisible();
	// «Nueva» became «Agregada» and the table re-laid itself: the amounts moved 19 px in place (pass 16).
	await expect.poll(edges).toEqual(before);
	await page.waitForTimeout(600);
	expect(await edges()).toEqual(before);
});
