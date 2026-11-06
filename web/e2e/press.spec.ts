// A press where nothing may slide (reduced motion), at the sizes people use: what it moves goes behind a fade, the page dims
// at most once, and a card's blocks never come or go in a frame. Through the browser, against a real server (see run.sh).
// Its own user: it makes rules and accounts by the dozen.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type BrowserContext, type Locator, type Page } from '@playwright/test';
import { expectPress, watchPress } from './frames.ts';
import { unfold } from './forms.ts';
import { say } from './messages.ts';
import { makeRules } from './rules.ts';
import { still } from './scroll.ts';
import { totp } from './totp.ts';

test.describe.configure({ mode: 'serial' });
test.use({ locale: 'es-PA' });
let session: Awaited<ReturnType<BrowserContext['storageState']>>;

test.beforeAll(async ({ browser }) => {
	const secret = process.env.E2E_PRESS_SECRET ?? '';
	expect(secret, 'run through e2e/run.sh').not.toBe('');
	const page = await browser.newPage({ locale: 'es-PA' });
	await page.goto('/accounts');
	await page.getByLabel(say('login.email')).fill(process.env.E2E_PRESS_EMAIL ?? '');
	await page.getByLabel(say('login.password')).fill(process.env.E2E_PRESS_PASSWORD ?? '');
	await page.getByLabel(say('login.code')).fill(totp(secret));
	await page.getByRole('button', { name: say('login.title') }).click();
	await expect(page.getByRole('heading', { name: say('accounts.title') })).toBeVisible();
	session = await page.context().storageState();
	await page.close();
});

async function opened(context: BrowserContext, width: number, height: number): Promise<Page> {
	await context.addCookies(session.cookies);
	const page = await context.newPage();
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.setViewportSize({ width, height });
	return page;
}

/** Presses `control` with the page at rest and judges what it moved once `done` holds. */
async function press(page: Page, what: string, control: Locator, done: () => Promise<void>) {
	await still(page);
	await watchPress(page);
	await control.first().dispatchEvent('click');
	await done();
	await expectPress(page, what);
}

type Shown = { shown: number[] };

/** Starts recording the opacity of `selector`, its parents' with it, while it stands. */
async function watchShown(page: Page, selector: string) {
	await page.evaluate((selector) => {
		const w = window as unknown as Shown;
		w.shown = [];
		const step = () => {
			let shown = document.querySelector(selector) ? 1 : 0;
			for (let e = document.querySelector(selector); e; e = e.parentElement)
				shown *= Number(getComputedStyle(e).opacity);
			w.shown.push(shown);
			if (w.shown.length < 60) requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	}, selector);
}

const account = (name: string) => ({
	name,
	kind: 'cash',
	currency: 'USD',
	namespace: name.toLowerCase().replaceAll(' ', '-')
});

test.describe.serial('every size', () => {
	for (const [width, height] of [
		[1280, 600],
		[1280, 800],
		[390, 600],
		[390, 844]
	] as const) {
		const size = `${width}x${height}`;

		test(`a new rule and a field's error at ${size}`, async ({ context }) => {
			const page = await opened(context, width, height);
			const made = await makeRules(page, [`Press A ${size}`, `Press B ${size}`]);
			await page.goto('/rules');
			const add = page.locator('#new-rule');
			if (width >= 900) {
				await add.click();
				const cancel = page.getByRole('button', { name: say('ruleform.cancel') });
				await press(page, `rule cancel ${size}`, cancel, () => expect(add).toBeFocused());
			}
			await add.click();
			await still(page);
			const send = page.locator(
				width < 900 ? 'dialog button[form="rule-form"]' : '#rule-form [type="submit"]'
			);
			await press(page, `rule error ${size}`, send, () =>
				expect(page.locator('#rule-form .field-error').first()).toBeVisible()
			);
			const form = page.locator('#rule-form');
			const name = `Press C ${size}`;
			await form.getByLabel(say('ruleform.name')).fill(name);
			await form.getByLabel(say('ruleform.text')).fill(name.toUpperCase());
			await unfold(page);
			await form.getByLabel(say('ruleform.payee')).selectOption('text');
			const row = page.locator('ol.rules > li', { hasText: name });
			await press(page, `new rule ${size}`, send, () => expect(row).toBeFocused());
			const check = page.locator('section.follows button.secondary');
			await press(page, `changes asked ${size}`, check, () =>
				expect(page.locator('#changes-none, #changes-some')).toBeFocused()
			);
			await page.getByRole('button', { name: say('rules.delete_named', { name }) }).click();
			const yes = page.getByRole('button', { name: say('rules.delete_it') });
			await press(page, `rule deleted ${size}`, yes, () => expect(row).toHaveCount(0));
			for (const rule of made)
				expect((await page.request.delete(`/api/v1/rules/${rule}`)).ok()).toBe(true);
		});

		test(`a new account and a field's error at ${size}`, async ({ context }) => {
			const page = await opened(context, width, height);
			for (const n of [1, 2, 3]) {
				const data = account(`Mid ${size} ${n}`);
				expect((await page.request.post('/api/v1/accounts', { data })).ok()).toBe(true);
			}
			await page.goto('/accounts');
			const add = page.locator('#new-account');
			await add.click();
			const cancel = page.getByRole('button', { name: say('accounts.cancel') });
			await press(page, `account cancel ${size}`, cancel, () => expect(add).toBeFocused());
			for (const name of [`Aaa ${size}`, `Zzz ${size}`]) {
				await add.click();
				await still(page);
				const send = page.locator('form:has(input[name="name"]) [type="submit"]');
				await press(page, `account error ${size}`, send, () =>
					expect(page.locator('form .field-error').first()).toBeVisible()
				);
				await page.getByLabel(say('accounts.name'), { exact: true }).fill(name);
				const made = page.locator('tr[data-account]', { hasText: name });
				await press(page, `new account ${name}`, send, () => expect(made).toBeFocused());
			}
		});

		test(`a refused sign-in at ${size}`, async ({ context }) => {
			const page = await context.newPage();
			await page.emulateMedia({ reducedMotion: 'reduce' });
			await page.setViewportSize({ width, height });
			await page.goto('/login');
			await page.getByLabel(say('login.email')).fill('nobody@example.com');
			await page.getByLabel(say('login.password')).fill('not the password');
			await page.getByLabel(say('login.code')).fill('000000');
			const send = page.getByRole('button', { name: say('login.title') });
			await press(page, `sign-in refused ${size}`, send, () =>
				expect(page.getByRole('alert').first()).toContainText(say('login.wrong'))
			);
		});

		test(`a category's error at ${size}`, async ({ context }) => {
			const page = await opened(context, width, height);
			await page.goto('/categories');
			const send = page.locator('#new-category [type="submit"]');
			const error = page.locator('#new-category .field-error').first();
			await press(page, `category error ${size}`, send, () => expect(error).toBeVisible());
			// A key that takes the error away moves little, and nothing that needs the whole page dim (pass 56).
			await still(page);
			await watchPress(page);
			await page.locator('#new-category input').first().fill(`Press ${size}`);
			await expect(error).toHaveCount(0);
			await expectPress(page, `category error gone ${size}`, 0);
			const made = page.getByText(`Press ${size}`, { exact: true });
			await press(page, `new category ${size}`, send, () => expect(made).toBeVisible());
		});
	}
});

const fixture = fileURLToPath(
	new URL('../../spec/fixtures/bac-account/input.csv', import.meta.url)
);
const statement = { name: 'estado.csv', mimeType: 'text/csv', buffer: readFileSync(fixture) };

for (const reducedMotion of ['no-preference', 'reduce'] as const) {
	test(`a change of account swaps what the card read behind a fade (${reducedMotion})`, async ({
		context
	}) => {
		const page = await opened(context, 1280, 800);
		await page.emulateMedia({ reducedMotion });
		const make = async (name: string, profile = 'bac-account') => {
			const data = { ...account(name), profile };
			const response = await page.request.post('/api/v1/accounts', { data });
			expect(response.ok()).toBe(true);
			return (await response.json()).id as string;
		};
		const [fresh, full] = [`Fresh ${reducedMotion}`, `Full ${reducedMotion}`];
		await make(fresh);
		const imported = await page.request.post(
			`/api/v1/accounts/${await make(full)}/imports?file_name=estado.csv`,
			{ data: statement.buffer, headers: { 'content-type': 'application/octet-stream' } }
		);
		expect(imported.ok()).toBe(true);
		await page.goto('/import');
		await page.locator('input[type=file]').setInputFiles(statement);
		const card = page.getByRole('article', { name: 'estado.csv' });
		const choice = card.getByLabel(say('file.account'));
		await choice.selectOption({ label: fresh });
		const button = card.getByRole('button', { name: /^Importar/ });
		await expect(button).toBeVisible();
		const block = 'article.card .reading :is(.actions, .notice, .hint, section)';
		const swaps = async (label: string, shows: () => Promise<void>) => {
			await still(page);
			await watchPress(page, block);
			await choice.selectOption({ label });
			await shows();
			await expectPress(page, `to ${label}`);
		};
		await swaps(full, () => expect(card.getByRole('status')).toBeVisible());
		await swaps(fresh, () => expect(button).toBeVisible());

		// To an account that cannot read it what it read goes, once, and comes back, once: it fell, came back with the old
		// figures and left, and came back twice on the way home (pass 56).
		const other = `Other ${reducedMotion}`;
		await make(other, 'bac-card');
		await page.reload();
		await page.locator('input[type=file]').setInputFiles(statement);
		await choice.selectOption({ label: fresh });
		await expect(button).toBeVisible();
		const once = async (label: string, shows: () => Promise<void>, way: 1 | -1) => {
			await still(page);
			await watchShown(page, '.reading');
			await watchPress(page);
			await choice.selectOption({ label });
			await shows();
			// With motion allowed it rises into place, a few pixels a frame.
			if (reducedMotion === 'reduce') await expectPress(page, `reading to ${label}`);
			await page.waitForFunction(() => (window as unknown as Shown).shown.length >= 60);
			const shown = await page.evaluate(() => (window as unknown as Shown).shown);
			const back = shown.slice(1).filter((now, i) => (now - shown[i]) * way < -0.02);
			expect(back, JSON.stringify(shown)).toEqual([]);
		};
		await once(other, () => expect(card.locator('.field-error')).toBeVisible(), -1);
		await once(fresh, () => expect(button).toBeVisible(), 1);
	});
}
