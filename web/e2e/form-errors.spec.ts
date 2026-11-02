// Form errors in Spanish, as a person meets them: nothing is left to the browser's own bubble (which speaks the
// browser's language, outside the page), each error sits under its field in the page's language with the field
// marked and focused, and no English detail from the API reaches the page; and where the page lands once a form is
// sent. Its own user (see run.sh).
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { say } from './messages.ts';
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
