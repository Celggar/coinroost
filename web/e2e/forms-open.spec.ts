// Forms on demand, as they open and close: in sight, and with what surrounds them moving calmly. Through the browser,
// against a real server (see run.sh). Its own user, whose household starts with nothing: the first account and the first
// rule need that.
import { expect, test, type BrowserContext, type Locator, type Page } from '@playwright/test';
import { expectFadedTop, expectGlidedTop, restTop, watchTop } from './frames.ts';
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
	const secret = process.env.E2E_OPEN_SECRET ?? '';
	expect(secret, 'run through e2e/run.sh').not.toBe('');
	const page = await browser.newPage({ locale: 'es-PA' });
	await page.goto('/accounts');
	await page.getByLabel(say('login.email')).fill(process.env.E2E_OPEN_EMAIL ?? '');
	await page.getByLabel(say('login.password')).fill(process.env.E2E_OPEN_PASSWORD ?? '');
	await page.getByLabel(say('login.code')).fill(totp(secret));
	await page.getByRole('button', { name: say('login.title') }).click();
	await expect(page.getByRole('heading', { name: say('accounts.title') })).toBeVisible();
	session = await page.context().storageState();
	await page.close();
});

async function loggedIn(context: BrowserContext): Promise<Page> {
	await context.addCookies(session.cookies);
	return context.newPage();
}

const inSight = (control: Locator) =>
	control.evaluate((e) => {
		const { top, bottom } = e.getBoundingClientRect();
		return top >= 0 && bottom <= innerHeight;
	});

async function forget(page: Page, rules: string[]) {
	for (const id of rules)
		expect((await page.request.delete(`/api/v1/rules/${id}`)).ok()).toBe(true);
}

test('the first account opens the list in a glide, the form not thrown down', async ({
	context
}) => {
	const page = await loggedIn(context);
	await page.goto('/accounts');
	const create = page.getByRole('button', { name: say('accounts.create') });
	await page.getByLabel(say('accounts.name'), { exact: true }).fill('First');
	await create.scrollIntoViewIfNeeded();
	await restTop(page, 'form');
	await watchTop(page, 'form');
	await create.dispatchEvent('click');
	await expect(page.getByRole('cell', { name: 'First', exact: true })).toBeVisible();
	await expectGlidedTop(page, 40);
});

for (const reducedMotion of ['no-preference', 'reduce'] as const) {
	test(`the first rule opens the list without a jump (${reducedMotion})`, async ({ context }) => {
		const page = await loggedIn(context);
		await page.emulateMedia({ reducedMotion });
		await page.setViewportSize({ width: 1280, height: 800 });
		await page.goto('/rules');
		const form = page.getByRole('form', { name: say('ruleform.title') });
		await form.getByLabel(say('ruleform.name')).fill('First');
		await form.getByLabel(say('ruleform.text')).fill('FIRST');
		await unfold(page);
		await form.getByLabel(say('ruleform.payee')).selectOption('text');
		const send = form.getByRole('button', { name: say('ruleform.add') });
		await send.scrollIntoViewIfNeeded();
		await restTop(page, 'section.follows');
		await watchTop(page, 'section.follows');
		await send.dispatchEvent('click');
		const row = page.locator('ol.rules > li', { hasText: 'First' });
		await expect(row).toBeFocused();
		if (reducedMotion === 'reduce') await expectFadedTop(page);
		else await expectGlidedTop(page, 80);
		await forget(page, [(await row.getAttribute('data-rule'))!]);
	});
}

test('an opened form stands in sight, and its button when it closes', async ({ context }) => {
	const page = await loggedIn(context);
	await page.setViewportSize({ width: 1280, height: 600 });
	const made = await makeRules(page, ['Sight A', 'Sight B', 'Sight C']);
	await page.goto('/rules');
	const add = page.locator('#new-rule');
	await add.click();
	const name = page.getByRole('form').getByLabel(say('ruleform.name'));
	await expect(name).toBeFocused();
	await expect.poll(() => inSight(name)).toBe(true);
	await page.getByRole('button', { name: say('ruleform.cancel') }).click();
	await expect(add).toBeFocused();
	await expect.poll(() => inSight(add)).toBe(true);
	await forget(page, made);

	await page.setViewportSize({ width: 390, height: 844 });
	for (const n of [2, 3, 4, 5, 6]) {
		const data = { name: `Sight ${n}`, kind: 'cash', currency: 'USD', namespace: `sight-${n}` };
		expect((await page.request.post('/api/v1/accounts', { data })).ok()).toBe(true);
	}
	await page.goto('/accounts');
	await page.locator('#new-account').click();
	const account = page.getByLabel(say('accounts.name'), { exact: true });
	await expect(account).toBeFocused();
	await expect.poll(() => inSight(account)).toBe(true);
});

test('under reduced motion a fold closing at the end moves nothing in plain sight', async ({
	context
}) => {
	const page = await loggedIn(context);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.setViewportSize({ width: 1280, height: 800 });
	const made = await makeRules(page, ['End A', 'End B', 'End C']);
	const closes = async (selector: string, shade?: string) => {
		// The form lands first: scrolled before that, its own landing took the page off the end.
		await still(page);
		await unfold(page);
		// Its room comes a frame after the click.
		await restTop(page, selector);
		await page.evaluate((s) => {
			const box = s ? document.querySelector(s)! : document.documentElement;
			box.scrollTop = box.scrollHeight;
		}, shade && 'dialog');
		await watchTop(page, selector, shade);
		await page.locator('form details summary').dispatchEvent('click');
		await expectFadedTop(page);
	};
	await page.goto('/rules');
	await page.locator('#new-rule').click();
	await closes('ol.rules');

	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto('/rules');
	await page.locator('#new-rule').click();
	await closes('dialog form input', 'dialog form');

	await page.goto('/accounts');
	await page.locator('#new-account').click();
	await closes('.table-wrap');
	await forget(page, made);
});

test('under reduced motion a form closing in a short window dims the page once', async ({
	context
}) => {
	const page = await loggedIn(context);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	const made = await makeRules(page, ['Short A', 'Short B', 'Short C']);
	// The page falls to its new end and goes on up to the button: one press dimmed it twice (pass 54).
	const closes = async (opener: string, list: string, cancel: string) => {
		await page.locator(opener).click();
		await still(page);
		await page.evaluate(() => scrollTo(0, document.documentElement.scrollHeight));
		await still(page);
		await watchTop(page, list);
		await page.getByRole('button', { name: cancel }).dispatchEvent('click');
		await expectFadedTop(page, 2, true);
	};
	await page.setViewportSize({ width: 1280, height: 600 });
	await page.goto('/rules');
	await closes('#new-rule', 'ol.rules', say('ruleform.cancel'));

	await page.setViewportSize({ width: 390, height: 600 });
	await page.goto('/accounts');
	await closes('#new-account', '.table-wrap', say('accounts.cancel'));
	await forget(page, made);
});

test('a rule moved under an answer leaves the block standing, the answer fading', async ({
	context
}) => {
	const page = await loggedIn(context);
	await page.setViewportSize({ width: 1280, height: 800 });
	const [first, second] = await makeRules(page, ['Moved A', 'Moved B']);
	await page.goto('/rules');
	const ask = page.locator('section.follows button.secondary');
	await expect(ask).toHaveText(say('changes.open'));
	type Words = { words: [string, number][] };
	await page.evaluate(() => {
		const w = window as unknown as Words;
		const button = document.querySelector('section.follows button.secondary')!;
		w.words = [];
		const step = () => {
			w.words.push([button.textContent!.trim(), Number(getComputedStyle(button).opacity)]);
			if (w.words.length < 60) requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	});
	await ask.dispatchEvent('click');
	const answer = page.locator('#changes-none');
	await expect(answer).toHaveCSS('opacity', '1');
	await page.waitForFunction(() => (window as unknown as Words).words.length >= 60);
	const words = await page.evaluate(() => (window as unknown as Words).words);
	// Its word changes once, out of sight: in a frame, the button went from 319 to 128 px at full opacity (pass 49).
	const changed = words.flatMap(([word, shown], i) =>
		i && word !== words[i - 1][0] ? [Math.max(shown, words[i - 1][1])] : []
	);
	expect(changed.length, JSON.stringify(words)).toBe(1);
	expect(changed[0], JSON.stringify(words)).toBeLessThan(0.3);
	await restTop(page, '#changes-none');
	await watchTop(page, '#changes-none');
	await page.locator(`[data-move="${first}-down"]`).dispatchEvent('click');
	await expect(answer).toHaveCount(0);
	await expect(page.getByRole('heading', { name: say('changes.title') })).toBeVisible();
	await expect(ask).toHaveText(say('changes.check'));
	await expectGlidedTop(page, 40);
	await forget(page, [first, second]);
});

test('at rest a fold cuts no ring of the fields at its edges', async ({ context }) => {
	const page = await loggedIn(context);
	await page.setViewportSize({ width: 1280, height: 800 });
	const made = await makeRules(page, ['Ring A']);
	await page.goto('/rules');
	await page.locator('#new-rule').click();
	await unfold(page);
	const clip = () =>
		page
			.locator('form details')
			.evaluate((fold) => getComputedStyle(fold, '::details-content').overflow);
	await expect.poll(clip).toBe('visible');
	await forget(page, made);
});

for (const reducedMotion of ['no-preference', 'reduce'] as const) {
	test(`in the phone's dialog an error at the top comes into sight (${reducedMotion})`, async ({
		context
	}) => {
		const page = await loggedIn(context);
		await page.emulateMedia({ reducedMotion });
		await page.setViewportSize({ width: 390, height: 844 });
		const made = await makeRules(page, ['Sheet error']);
		await page.goto('/rules');
		await page.locator('#new-rule').click();
		const sheet = page.getByRole('dialog', { name: say('ruleform.title') });
		const name = sheet.getByLabel(say('ruleform.name'));
		await expect(name).toBeFocused();
		await unfold(page);
		await restTop(page, 'dialog form input');
		await page.evaluate(() => {
			const box = document.querySelector('dialog')!;
			box.scrollTop = box.scrollHeight;
		});
		await watchTop(page, 'dialog form input', 'dialog form');
		await sheet.getByRole('button', { name: say('ruleform.save') }).dispatchEvent('click');
		await expect(name).toBeFocused();
		if (reducedMotion === 'reduce') await expectFadedTop(page);
		// The field itself at its centre, not the header that stood over it.
		await expect
			.poll(() =>
				name.evaluate((e) => {
					const { left, top, width, height } = e.getBoundingClientRect();
					return document.elementFromPoint(left + width / 2, top + height / 2) === e;
				})
			)
			.toBe(true);
		await forget(page, made);
	});
}

for (const reducedMotion of ['no-preference', 'reduce'] as const) {
	for (const width of [390, 1280]) {
		test(`a rule deleted near the page's end moves nothing at once (${reducedMotion}, ${width})`, async ({
			context
		}) => {
			const page = await loggedIn(context);
			await page.emulateMedia({ reducedMotion });
			await page.setViewportSize({ width, height: 800 });
			const names = ['Gone A', 'Gone B', 'Gone C'].map(
				(name) => `${name} ${width} ${reducedMotion}`
			);
			const made = await makeRules(page, names);
			await page.goto('/rules');
			const ask = page.getByRole('button', { name: say('rules.delete_named', { name: names[1] }) });
			await expect(ask).toBeVisible();
			await still(page);
			await ask.evaluate((e) => e.scrollIntoView({ block: 'center', behavior: 'instant' }));
			await ask.dispatchEvent('click');
			const yes = page.getByRole('button', { name: say('rules.delete_it') });
			await expect(yes).toBeVisible();
			await restTop(page, 'ol.rules');
			await watchTop(page, 'ol.rules');
			await yes.dispatchEvent('click');
			await expect(page.getByRole('heading', { name: names[1] })).toHaveCount(0);
			if (reducedMotion === 'reduce') await expectFadedTop(page);
			else await expectGlidedTop(page, 60);
			await forget(page, [made[0], made[2]]);
		});
	}
}

for (const width of [390, 1280]) {
	// Between: neither the answer's room nor the rule's reaches the end alone, both together do.
	for (const [count, between] of [
		[0, false],
		[6, false],
		[6, true]
	] as const) {
		test(`under reduced motion a rule deleted over an answer of ${count} moves the end behind a fade (${width}${between ? ', between' : ''})`, async ({
			context
		}) => {
			const page = await loggedIn(context);
			await page.emulateMedia({ reducedMotion: 'reduce' });
			await page.setViewportSize({ width, height: 800 });
			const names = ['Over A', 'Over B', 'Over C'].map(
				(name) => `${name} ${count} ${width} ${between}`
			);
			const made = await makeRules(page, names);
			const row = (n: number) => ({
				transaction_id: `00000000-0000-4000-8000-00000000000${n}`,
				date: `2026-05-0${n}`,
				description: `Row ${n}`,
				amount_minor: -1000 * n,
				currency: 'USD',
				before: { category_id: null, payee_id: null },
				after: { category_id: null, payee_id: null }
			});
			const changes = [1, 2, 3, 4, 5, 6].slice(0, count).map(row);
			await page.route('**/api/v1/rules/preview', (route) =>
				route.fulfill({ json: { tag: 'tag', changes } })
			);
			await page.goto('/rules');
			await page.locator('section.follows button.secondary').click();
			await expect(page.locator(count ? '#changes-some' : '#changes-none')).toBeVisible();
			await still(page);
			await page
				.getByRole('button', { name: say('rules.delete_named', { name: names[1] }) })
				.dispatchEvent('click');
			const yes = page.getByRole('button', { name: say('rules.delete_it') });
			await expect(yes).toBeVisible();
			await still(page);
			await page.evaluate((between) => {
				const height = (selector: string) =>
					document.querySelector<HTMLElement>(selector)!.offsetHeight;
				const room = Math.max(height('section.follows .in-place'), height('ol.rules > li') + 24);
				scrollTo(
					0,
					document.documentElement.scrollHeight - innerHeight - (between ? room + 20 : 0)
				);
			}, between);
			await restTop(page, 'ol.rules');
			await watchTop(page, 'ol.rules');
			await yes.dispatchEvent('click');
			await expect(page.getByRole('heading', { name: names[1] })).toHaveCount(0);
			await expectFadedTop(page);
			await page.unroute('**/api/v1/rules/preview');
			await forget(page, [made[0], made[2]]);
		});
	}
}

test('on a phone asking to delete a rule moves nothing under it', async ({ context }) => {
	const page = await loggedIn(context);
	await page.setViewportSize({ width: 390, height: 800 });
	const names = ['Ask A', 'Ask B', 'Ask C'];
	const made = await makeRules(page, names);
	await page.goto('/rules');
	const ask = page.getByRole('button', { name: say('rules.delete_named', { name: 'Ask B' }) });
	await expect(ask).toBeVisible();
	await still(page);
	// Where a person could press it: below the window, the focus on its answer scrolled the page.
	await ask.evaluate((e) => e.scrollIntoView({ block: 'center', behavior: 'instant' }));
	await watchTop(page, 'ol.rules > li:nth-child(3)');
	await ask.dispatchEvent('click');
	const keep = page.getByRole('button', { name: say('rules.keep_it') });
	await expect(keep).toBeVisible();
	await expectGlidedTop(page, 4);
	await watchTop(page, 'ol.rules > li:nth-child(3)');
	await keep.dispatchEvent('click');
	await expect(ask).toBeVisible();
	await expectGlidedTop(page, 4);
	await forget(page, made);
});

for (const reducedMotion of ['no-preference', 'reduce'] as const) {
	test(`a new category opens its room, the form not thrown down (${reducedMotion})`, async ({
		context
	}) => {
		const page = await loggedIn(context);
		await page.emulateMedia({ reducedMotion });
		await page.setViewportSize({ width: 1280, height: 800 });
		await page.goto('/categories');
		const form = page.locator('#new-category');
		const name = form.getByLabel(say('categoryform.name'));
		const make = async (category: string) => {
			await name.fill(category);
			await restTop(page, '#new-category');
			const faded = reducedMotion === 'reduce';
			const first = (await page.locator('ul.list > li').count()) === 0;
			await watchTop(page, '#new-category', faded && !first ? '#new-category' : 'main');
			await form.getByRole('button', { name: say('categoryform.create') }).dispatchEvent('click');
			await expect(page.getByText(category, { exact: true })).toBeVisible();
			if (faded) await expectFadedTop(page);
			else await expectGlidedTop(page, 20);
		};
		await make(`Aaa first ${reducedMotion}`);
		await make(`Bbb next ${reducedMotion}`);
	});
}
