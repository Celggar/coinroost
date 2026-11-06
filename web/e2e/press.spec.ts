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
async function press(
	page: Page,
	what: string,
	control: Locator | (() => Promise<unknown>),
	done: () => Promise<void>
) {
	await still(page);
	// What the last key moved may still be fading: that dim is not this press's.
	await page.waitForFunction(() => !document.getAnimations().some((a) => a.id === 'page'));
	await watchPress(page);
	if (typeof control === 'function') await control();
	else await control.first().dispatchEvent('click');
	await done();
	await expectPress(page, what);
}

type Shown = { shown: Record<string, number[]> };

/** Starts recording the opacity of `selector`, its parents' with it, while it stands. */
async function watchShown(page: Page, selector: string) {
	await page.evaluate((selector) => {
		const w = window as unknown as Shown;
		w.shown ??= {};
		const shown: number[] = (w.shown[selector] = []);
		const step = () => {
			let now = document.querySelector(selector) ? 1 : 0;
			for (let e = document.querySelector(selector); e; e = e.parentElement)
				now *= Number(getComputedStyle(e).opacity);
			shown.push(now);
			if (shown.length < 60) requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	}, selector);
}

/** What `watchShown` saw of each selector, once it has seen enough: it may fall, then rise, not more. */
async function expectOneFade(page: Page) {
	await page.waitForFunction(() =>
		Object.values((window as unknown as Shown).shown).every((shown) => shown.length >= 60)
	);
	const seen = await page.evaluate(() => (window as unknown as Shown).shown);
	for (const [selector, shown] of Object.entries(seen)) {
		const steps = shown.slice(1).map((now, i) => now - shown[i]);
		const rose = steps.findIndex((step) => step > 0.02);
		const again = rose < 0 ? [] : steps.slice(rose).filter((step) => step < -0.02);
		expect(again, `${selector}: ${JSON.stringify(shown)}`).toEqual([]);
	}
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
			// The rows standing above it fall and rise once, whatever the page does around them (pass 57).
			await page.evaluate(() => ((window as unknown as Shown).shown = {}));
			await watchShown(page, 'ol.rules > li');
			await press(page, `new rule ${size}`, send, () => expect(row).toBeFocused());
			await expectOneFade(page);
			const check = page.locator('section.follows button.secondary');
			await press(page, `changes asked ${size}`, check, () =>
				expect(page.locator('#changes-none, #changes-some')).toBeFocused()
			);
			// Moved near the top a landing follows, and the rows must not fall for each (pass 58).
			await page.evaluate(() => scrollTo(0, 0));
			const down = page.locator(`[data-move="${made[0]}-down"]`);
			await press(page, `rule moved ${size}`, down, () => expect(down).toBeFocused());
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
			for (const n of [1, 2, 3, 4, 5, 6]) {
				const data = { name: `Many ${size} ${n}`, kind: 'expense' };
				expect((await page.request.post('/api/v1/categories', { data })).ok()).toBe(true);
			}
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
			const name = page.locator('#new-category input').first();
			await press(
				page,
				`new category ${size}`,
				() => name.press('Enter'),
				() => expect(made).toBeVisible()
			);
		});
	}
});

for (const [width, height] of [
	[1280, 600],
	[390, 600],
	[390, 844]
] as const) {
	test(`choices in the queue at ${width}x${height}`, async ({ context }) => {
		const page = await opened(context, width, height);
		const data = account(`Queue ${width}x${height}`);
		const made = await page.request.post('/api/v1/accounts', { data });
		expect(made.ok()).toBe(true);
		const { id } = await made.json();
		for (const n of [1, 2, 3, 4, 5, 6, 7, 8]) {
			// One line that takes a second with what it took, one that has two already (pass 58).
			const description = n % 2 ? `A RATHER LONGER ONE OF A KIND ${n}` : `CAFE EJEMPLO ${n}`;
			const legs = [{ account_id: id, amount_minor: -100 * n, statement_text: description }];
			const row = { date: '2026-05-10', description, legs };
			expect((await page.request.post('/api/v1/transactions', { data: row })).ok()).toBe(true);
		}
		await page.goto('/uncategorized');
		for (const round of [1, 2, 3, 4]) {
			const select = page.locator('.queue select:not([disabled])').first();
			await expect(select).toBeVisible();
			const row = select.locator('xpath=ancestor::li');
			await press(
				page,
				`choice ${round} at ${width}x${height}`,
				() => select.selectOption({ index: 1 }),
				() => expect(row.getByRole('button', { name: say('uncategorized.undo') })).toBeVisible()
			);
		}
	});
}

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
		// figures and left, and came back twice on the way home (pass 56). The next card goes behind one fade, not two with a
		// jump between: 355 px at half its opacity (pass 57).
		const other = `Other ${reducedMotion}`;
		await make(other, 'bac-card');
		await page.reload();
		await page
			.locator('input[type=file]')
			.setInputFiles([statement, { ...statement, name: 'siguiente.csv' }]);
		await choice.selectOption({ label: fresh });
		await expect(button).toBeVisible();
		const next = 'article.card[aria-label="siguiente.csv"]';
		await expect(page.locator(next)).toBeVisible();
		const once = async (label: string, shows: () => Promise<void>) => {
			await still(page);
			await page.evaluate(() => ((window as unknown as Shown).shown = {}));
			await watchShown(page, '.reading');
			await watchShown(page, next);
			await watchPress(page);
			await choice.selectOption({ label });
			await shows();
			// With motion allowed it rises into place, a few pixels a frame.
			if (reducedMotion === 'reduce') await expectPress(page, `reading to ${label}`);
			await expectOneFade(page);
		};
		await once(other, () => expect(card.locator('.field-error')).toBeVisible());
		await once(fresh, () => expect(button).toBeVisible());
	});
}

for (const [width, height] of [
	[1280, 800],
	[390, 844]
] as const) {
	test(`a card taken away from the page's end at ${width}x${height}`, async ({ context }) => {
		const page = await opened(context, width, height);
		await page.goto('/import');
		await page
			.locator('input[type=file]')
			.setInputFiles(['uno.csv', 'dos.csv', 'tres.csv'].map((name) => ({ ...statement, name })));
		const remove = page.getByRole('button', {
			name: say('file.remove_named', { name: 'uno.csv' })
		});
		await expect(remove).toBeVisible();
		await still(page);
		await page.evaluate(() => scrollTo(0, document.documentElement.scrollHeight));
		await press(page, `card removed ${width}`, remove, () => expect(remove).toHaveCount(0));
	});
}
