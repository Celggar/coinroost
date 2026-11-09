// A press at the sizes people use, in either motion setting. Where nothing may slide what it moves goes behind a fade, the page
// dims at most once, and a card's blocks never come or go in a frame; with motion allowed nothing moves in a single frame.
// Through the browser, against a real server (see run.sh). Each setting has its own user: it makes rules and accounts by the
// dozen.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type BrowserContext, type Locator, type Page } from '@playwright/test';
import { expectNotCut, expectPress, watchCut, watchPress } from './frames.ts';
import { unfold } from './forms.ts';
import { say } from './messages.ts';
import { makeRules } from './rules.ts';
import { settled, still } from './scroll.ts';
import { totp } from './totp.ts';

export function pressCases(motion: 'reduce' | 'no-preference', user: 'PRESS' | 'GLIDE') {
	test.describe.configure({ mode: 'serial' });
	test.use({ locale: 'es-PA' });
	let session: Awaited<ReturnType<BrowserContext['storageState']>>;

	test.beforeAll(async ({ browser }) => {
		const secret = process.env[`E2E_${user}_SECRET`] ?? '';
		expect(secret, 'run through e2e/run.sh').not.toBe('');
		const page = await browser.newPage({ locale: 'es-PA' });
		await page.goto('/accounts');
		await page.getByLabel(say('login.email')).fill(process.env[`E2E_${user}_EMAIL`] ?? '');
		await page.getByLabel(say('login.password')).fill(process.env[`E2E_${user}_PASSWORD`] ?? '');
		await page.getByLabel(say('login.code')).fill(totp(secret));
		await page.getByRole('button', { name: say('login.title') }).click();
		await expect(page.getByRole('heading', { name: say('accounts.title') })).toBeVisible();
		session = await page.context().storageState();
		await page.close();
	});

	async function opened(context: BrowserContext, width: number, height: number): Promise<Page> {
		await context.addCookies(session.cookies);
		const page = await context.newPage();
		await page.emulateMedia({ reducedMotion: motion });
		await page.setViewportSize({ width, height });
		return page;
	}

	// With motion, how far a line's glide may step: the main thread stalled 80 ms as it began, and 13 px of its 24 fell in one
	// frame, 38 of two lines' 48. Given back in a frame it is the whole line.
	const LINE = 20;
	const LINES = 40;
	// D65: a form closing with little scroll left takes the page along its curve, up to 66 px a frame, cut where the scroll ends.
	const closing = (width: number) => (width >= 900 ? 70 : undefined);

	/** Presses `control` with the page at rest and judges what it moved once `done` holds. */
	async function press(
		page: Page,
		what: string,
		control: Locator | (() => Promise<unknown>),
		done: () => Promise<void>,
		lone?: number
	) {
		await still(page);
		// What the last key moved may still be fading: that dim is not this press's.
		await page.waitForFunction(() => !document.getAnimations().some((a) => a.id === 'page'));
		await watchPress(page, '', lone);
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
					const back = () => expect(add).toBeFocused();
					await press(page, `rule cancel ${size}`, cancel, back, closing(width));
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
				const ask = async () => {
					await watchCut(page, 'section.follows button.secondary', 'section.follows');
					await check.dispatchEvent('click');
				};
				await press(page, `changes asked ${size}`, ask, () =>
					expect(page.locator('#changes-none, #changes-some')).toBeFocused()
				);
				await expectNotCut(page, `changes asked ${size}`);
				// Moved near the top a landing follows, and the rows must not fall for each (pass 58).
				await page.evaluate(() => scrollTo(0, 0));
				const down = page.locator(`[data-move="${made[0]}-down"]`);
				await press(page, `rule moved ${size}`, down, () => expect(down).toBeFocused());
				const remove = page.getByRole('button', { name: say('rules.delete_named', { name }) });
				const keep = page.getByRole('button', { name: say('rules.keep_it') });
				await remove.scrollIntoViewIfNeeded();
				await press(page, `delete asked ${size}`, remove, () => expect(keep).toBeFocused());
				await press(page, `rule kept ${size}`, keep, () => expect(remove).toBeFocused());
				await remove.click();
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
				const back = () => expect(add).toBeFocused();
				await press(page, `account cancel ${size}`, cancel, back, closing(width));
				// The long one: a table laid out by what it holds moves every column for it.
				const long = `Zzz ${size} with a name long enough to widen its own column`;
				for (const name of [`Aaa ${size}`, long]) {
					await add.click();
					await still(page);
					const send = page.locator('form:has(input[name="name"]) [type="submit"]');
					await press(page, `account error ${size}`, send, () =>
						expect(page.locator('form .field-error').first()).toBeVisible()
					);
					await page.getByLabel(say('accounts.name'), { exact: true }).fill(name);
					const made = page.locator('tr[data-account]', { hasText: name });
					const shown = () => expect(made).toBeFocused();
					await press(page, `new account ${name}`, send, shown, closing(width));
				}
			});

			test(`a refused sign-in at ${size}`, async ({ context }) => {
				const page = await context.newPage();
				await page.emulateMedia({ reducedMotion: motion });
				await page.setViewportSize({ width, height });
				await page.goto('/login');
				await page.getByLabel(say('login.email')).fill('nobody@example.com');
				await page.getByLabel(say('login.password')).fill('not the password');
				await page.getByLabel(say('login.code')).fill('000000');
				const send = page.getByRole('button', { name: say('login.title') });
				// With motion the card, centred, glides 25 px up to the page's padding: most of that can fall in one frame.
				const refused = () =>
					expect(page.getByRole('alert').first()).toContainText(say('login.wrong'));
				await press(page, `sign-in refused ${size}`, send, refused, 32);
			});

			test(`the login's second step at ${size}`, async ({ context }) => {
				const page = await context.newPage();
				await page.emulateMedia({ reducedMotion: motion });
				await page.setViewportSize({ width, height });
				await page.goto('/login');
				const swap = (key: 'login.use_recovery' | 'login.use_code') =>
					page.getByRole('button', { name: say(key), exact: true });
				await expect(swap('login.use_recovery')).toBeVisible();
				const line = (key: 'login.subtitle' | 'login.recovery_subtitle') => () =>
					expect(page.getByText(say(key), { exact: true })).toBeVisible();
				await press(
					page,
					`login to recovery ${size}`,
					swap('login.use_recovery'),
					line('login.recovery_subtitle')
				);
				await press(page, `login to code ${size}`, swap('login.use_code'), line('login.subtitle'));
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
					() => expect(row.getByRole('button', { name: say('uncategorized.undo') })).toBeVisible(),
					LINE
				);
			}
		});
	}

	const fixture = fileURLToPath(
		new URL('../../spec/fixtures/bac-account/input.csv', import.meta.url)
	);
	const statement = { name: 'statement.csv', mimeType: 'text/csv', buffer: readFileSync(fixture) };

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
				`/api/v1/accounts/${await make(full)}/imports?file_name=statement.csv`,
				{ data: statement.buffer, headers: { 'content-type': 'application/octet-stream' } }
			);
			expect(imported.ok()).toBe(true);
			await page.goto('/import');
			await page.locator('input[type=file]').setInputFiles(statement);
			const card = page.getByRole('article', { name: 'statement.csv' });
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
				.setInputFiles([statement, { ...statement, name: 'next.csv' }]);
			await choice.selectOption({ label: fresh });
			await expect(button).toBeVisible();
			const next = 'article.card[aria-label="next.csv"]';
			await expect(page.locator(next)).toBeVisible();
			const once = async (label: string, shows: () => Promise<void>) => {
				// The next card may still be fading in: that rise was its arrival's, and the swap's drop after it read as a second
				// fade (D77, from 0.56 to 0.83). Watching starts once it has arrived.
				await Promise.all([still(page), settled(page)]);
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
				.setInputFiles(['one.csv', 'two.csv', 'three.csv'].map((name) => ({ ...statement, name })));
			const remove = page.getByRole('button', {
				name: say('file.remove_named', { name: 'one.csv' })
			});
			await expect(remove).toBeVisible();
			await still(page);
			await page.evaluate(() => scrollTo(0, document.documentElement.scrollHeight));
			await press(page, `card removed ${width}`, remove, () => expect(remove).toHaveCount(0));
		});
	}

	/** Rows of an account of their own, in the order of `rows`. */
	async function seeded(page: Page, tag: string, rows: { description: string; date: string }[]) {
		const made = await page.request.post('/api/v1/accounts', { data: account(`Rows ${tag}`) });
		expect(made.ok()).toBe(true);
		const { id } = await made.json();
		for (const [n, { description, date }] of rows.entries()) {
			const legs = [{ account_id: id, amount_minor: -100 * (n + 1), statement_text: description }];
			const row = { date, description, legs };
			expect((await page.request.post('/api/v1/transactions', { data: row })).ok()).toBe(true);
		}
	}

	async function category(page: Page, name: string) {
		const data = { name, kind: 'expense' };
		expect((await page.request.post('/api/v1/categories', { data })).ok()).toBe(true);
	}

	const others = (tag: string, date: string) =>
		[1, 2, 3, 4, 5, 6].map((n) => ({ description: `OTHER ${tag} ${n}`, date }));

	for (const [width, height] of [
		[1280, 800],
		[390, 844]
	] as const) {
		test(`a first choice whose words take a line, and back, at ${width}x${height}`, async ({
			context
		}) => {
			const page = await opened(context, width, height);
			const size = `${width}x${height}`;
			const name = `Subscriptions and digital services of the home ${size}`;
			await category(page, name);
			const description = width < 900 ? `CASH ${size}` : `A DEPOSIT IN CASH AT THE BRANCH ${size}`;
			await seeded(page, size, [
				{ description, date: '2026-12-31' },
				...others(size, '2026-12-30')
			]);
			await page.goto('/uncategorized');
			const row = page.locator('.queue > li', { hasText: description });
			const undo = row.getByRole('button', { name: say('uncategorized.undo') });
			const words = row.locator('.description');
			const before = (await words.boundingBox())?.height ?? 0;
			// The rows below fell for the fade it took, and again for one left waiting under it (pass 59).
			await press(
				page,
				`first choice ${size}`,
				() => row.locator('select').selectOption({ label: name }),
				() => expect(undo).toBeVisible(),
				LINE
			);
			expect((await words.boundingBox())?.height).toBeGreaterThan(before + 3);
			// With the offer open it gives back the offer's room and a line: −24 px at full opacity (pass 59).
			await press(page, `undone ${size}`, undo, () => expect(row.locator('select')).toBeFocused());
		});
	}

	for (const [width, height] of [
		[1280, 800],
		[390, 600],
		[390, 844]
	] as const) {
		test(`changes applied from the offer at ${width}x${height}`, async ({ context }) => {
			const page = await opened(context, width, height);
			const size = `${width}x${height}`;
			const name = `Applied ${size}`;
			await category(page, name);
			const shop = `SAME SHOP ${size}`;
			await seeded(page, `applied ${size}`, [
				...others(`applied ${size}`, '2026-12-29'),
				...[1, 2, 3].map((n) => ({ description: shop, date: `2026-01-0${n}` }))
			]);
			await page.goto('/uncategorized');
			const select = page.locator('.queue > li', { hasText: shop }).first().locator('select');
			await select.scrollIntoViewIfNeeded();
			await select.selectOption({ label: name });
			await page.getByRole('button', { name: say('uncategorized.make_rule') }).click();
			const apply = page.getByRole('button', { name: say('changes.apply_other', { count: 2 }) });
			await expect(apply).toBeVisible();
			// The list fell behind its fade, and with the page as the landing dimmed it after (pass 59).
			await press(page, `changes applied ${size}`, apply, () =>
				expect(page.locator('.queue select').first()).toBeFocused()
			);
		});
	}

	type Together = { together: number; counted: number };

	/** Starts counting how many of `selector` show at once, at most. */
	async function watchTogether(page: Page, selector: string) {
		await page.evaluate((selector) => {
			const w = window as unknown as Together;
			w.together = 0;
			w.counted = 0;
			const step = () => {
				const showing = [...document.querySelectorAll(selector)].filter(
					(e) => Number(getComputedStyle(e).opacity) > 0.1
				);
				w.together = Math.max(w.together, showing.length);
				if (++w.counted < 60) requestAnimationFrame(step);
			};
			requestAnimationFrame(step);
		}, selector);
	}

	for (const reducedMotion of ['no-preference', 'reduce'] as const) {
		test(`what Security swaps takes turns (${reducedMotion})`, async ({ context }) => {
			const page = await opened(context, 1280, 800);
			await page.emulateMedia({ reducedMotion });
			await page.goto('/security');
			const block = '.cell > .mode';
			const turn = async (control: Locator, shows: Locator) => {
				await still(page);
				await watchTogether(page, block);
				await control.dispatchEvent('click');
				await expect(shows).toBeVisible();
				await page.waitForFunction(() => (window as unknown as Together).counted >= 60);
				// The new block stood over the old one, both at 0.8 for 120 ms where nothing may slide (pass 59).
				expect(await page.evaluate(() => (window as unknown as Together).together)).toBe(1);
			};
			const ask = page.locator(`${block} > button.primary`);
			const cancel = page.getByRole('button', { name: say('security.cancel') });
			await turn(ask, cancel);
			await turn(cancel, ask);
		});
	}

	for (const [width, height] of [
		[1280, 800],
		[390, 844]
	] as const) {
		test(`out of «Otro…» to other figures at ${width}x${height}`, async ({ context }) => {
			const page = await opened(context, width, height);
			await page.goto('/');
			const preset = (name: string) => page.locator('.presets button', { hasText: name });
			const dates = page.locator('#period-dates');
			await expect(page.locator('.reports')).toBeVisible();
			await preset(say('dashboard.this_month')).click();
			await preset(say('dashboard.other')).click();
			await expect(dates).toBeVisible();
			// All that follows the period stood blank for 300 ms and came back in a frame (pass 60).
			await press(page, `out of Otro ${width}`, preset(say('dashboard.twelve')), () =>
				expect(dates).toHaveCount(0)
			);
		});
	}

	if (motion === 'no-preference') {
		test('what a row took comes and goes in a glide, at the end of the page', async ({
			context
		}) => {
			const page = await opened(context, 390, 844);
			const name = 'Subscriptions and digital services of the home, at the end';
			await category(page, name);
			const description = 'CASH AT THE END';
			await seeded(page, 'end', [{ description, date: '2025-12-31' }]);
			await page.goto('/uncategorized');
			const row = page.locator('.queue > li', { hasText: description });
			const select = row.locator('select');
			const undo = row.getByRole('button', { name: say('uncategorized.undo') });
			await select.scrollIntoViewIfNeeded();
			await still(page);
			// They came and went in a frame (pass 61).
			await watchPress(page, '.took', LINES);
			await select.selectOption({ label: name });
			await expect(undo).toBeVisible();
			await expectPress(page, 'took', 0);
			await page.getByRole('button', { name: say('uncategorized.only_this') }).click();
			await expect(page.getByRole('textbox', { name: say('uncategorized.offer') })).toHaveCount(0);
			await still(page);
			await page.evaluate(() => scrollTo(0, document.documentElement.scrollHeight));
			await still(page);
			// The row gave back a line in a frame, and the page its end with it: the whole screen moved 48 px (pass 61).
			await watchPress(page, '.took', LINES);
			await undo.dispatchEvent('click');
			await expect(select).toBeVisible();
			await expectPress(page, 'undone', 0);
		});
	}
}
