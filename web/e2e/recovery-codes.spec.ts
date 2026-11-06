// Recovery codes through the browser, against a real server (see run.sh): a setup link hands out ten codes, which
// copy and download; the Security page counts them; a login with one leads to a new authenticator and nowhere else;
// a new set retires the old one.
import { expect, test, type Page } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { expectGlidedTop, watchTop } from './frames.ts';
import { endsAlone } from './lines.ts';
import { say, type Key } from './messages.ts';
import { logOut } from './account.ts';
import { still } from './scroll.ts';
import { totp } from './totp.ts';

const invite = process.env.E2E_RECOVERY_INVITE ?? '';
const email = `recovery-${Date.now()}@example.com`;
const password = 'a long enough password';

test.use({ locale: 'es-PA', permissions: ['clipboard-read', 'clipboard-write'] });

/** The Security page's count line, whatever the date. */
function left(key: Key, count: number): RegExp {
	const text = say(key, { count, size: 10, date: 'DATE' });
	return new RegExp(text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').replace('DATE', '.+'));
}

async function shownCodes(page: Page): Promise<string[]> {
	const items = page.getByRole('list', { name: say('recovery.list') }).getByRole('listitem');
	await expect(items).toHaveCount(10);
	return (await items.allInnerTexts()).map((text) => text.trim());
}

async function logInWithRecovery(page: Page, code: string) {
	await page.getByLabel(say('login.email')).fill(email);
	await page.getByLabel(say('login.password')).fill(password);
	const field = page.getByLabel(say('login.recovery_code'));
	if (!(await field.isVisible())) {
		await page.getByRole('button', { name: say('login.use_recovery'), exact: true }).click();
		// The pressed button leaves with the old field: the caret goes to the new one.
		await expect(field).toBeFocused();
	}
	await field.fill(code);
	await page.getByRole('button', { name: say('login.title'), exact: true }).click();
}

test('recovery codes let a person back in and make them set up a new authenticator', async ({
	page
}) => {
	expect(invite, 'run through e2e/run.sh').not.toBe('');
	await page.goto(`/setup#${invite}`);
	await page.getByLabel(say('login.email')).fill(email);
	await page.getByLabel(say('login.password')).fill(password);
	await page.getByRole('button', { name: say('setup.continue'), exact: true }).click();
	const secret = (await page.getByTestId('setup-key').innerText()).replace(/\s/g, '');
	// On a 360 px phone the key's label wraps with its last two words together: «configuración» was alone (pass 22).
	await page.setViewportSize({ width: 360, height: 800 });
	expect(await endsAlone(page.getByText(say('setup.key')))).toBe(false);
	await page.setViewportSize({ width: 1280, height: 720 });
	// Under forced colors «Atrás», a button dressed as a link, keeps the links' colour (pass 23: ButtonText beside
	// LinkText on /authenticator, which shares the style).
	await page.emulateMedia({ forcedColors: 'active' });
	const colours = await page
		.getByRole('button', { name: say('setup.back'), exact: true })
		.evaluate((back) => {
			const link = document.body.appendChild(document.createElement('a'));
			link.href = '#';
			link.textContent = 'link';
			const pair = [getComputedStyle(back).color, getComputedStyle(link).color];
			link.remove();
			return pair;
		});
	expect(colours[0]).toBe(colours[1]);
	await page.emulateMedia({ forcedColors: 'none' });
	// A clipboard that refuses (a phone without the permission): the page says so and selects the key for a copy by
	// hand. It said nothing (pass 22).
	type Held = Clipboard & { real?: Clipboard['writeText'] };
	await page.evaluate(() => {
		const clipboard = navigator.clipboard as Held;
		clipboard.real = clipboard.writeText.bind(clipboard);
		clipboard.writeText = () => Promise.reject(new DOMException('Denied', 'NotAllowedError'));
	});
	await page.getByRole('button', { name: say('setup.copy'), exact: true }).click();
	await expect(page.locator('.key p.field-error')).toHaveText(say('setup.copy_failed'));
	expect(await page.evaluate(() => getSelection()?.toString().replace(/\s/g, ''))).toBe(secret);
	await page.evaluate(() => {
		const clipboard = navigator.clipboard as Held;
		clipboard.writeText = clipboard.real!;
	});
	await page.getByLabel(say('login.code'), { exact: true }).fill(totp(secret));
	await page.getByRole('button', { name: say('setup.finish'), exact: true }).click();

	// Step 3: ten codes, which copy and download as they are shown.
	await expect(page.getByRole('heading', { name: say('setup.step_codes') })).toBeVisible();
	const codes = await shownCodes(page);
	expect(
		codes.every((code) => /^[0-9a-z]{5}-[0-9a-z]{5}$/.test(code)),
		codes.join()
	).toBe(true);
	await page.getByTestId('recovery-copy').click();
	await expect(page.getByTestId('recovery-copy')).toHaveText(say('recovery.copied_short'));
	expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(codes.join('\n'));
	const [download] = await Promise.all([
		page.waitForEvent('download'),
		page.getByRole('button', { name: say('recovery.download'), exact: true }).click()
	]);
	expect(download.suggestedFilename()).toBe('coinroost-recovery-codes.txt');
	const file = await readFile(await download.path(), 'utf8');
	for (const code of [...codes, email]) expect(file).toContain(code);
	await page.getByRole('button', { name: say('setup.codes_done'), exact: true }).click();

	await page.getByRole('link', { name: say('nav.security') }).click();
	await expect(page.getByTestId('recovery-status')).toHaveText(
		left('security.codes_left_other', 10)
	);

	// A lost phone: a recovery code, typed loosely, leads to a new authenticator and nowhere else.
	await logOut(page);
	await expect(page).toHaveURL(/\/login/);
	await logInWithRecovery(page, codes[0].toUpperCase().replace('-', ' '));
	await expect(page).toHaveURL(/\/authenticator$/);
	await expect(page.getByRole('heading', { name: say('authenticator.title') })).toBeVisible();
	await page.goto('/accounts');
	await expect(page).toHaveURL(/\/authenticator$/);
	const fresh = (await page.getByTestId('setup-key').innerText()).replace(/\s/g, '');
	expect(fresh).not.toBe(secret);
	await page.getByLabel(say('login.code'), { exact: true }).fill(totp(secret, Date.now() + 30_000));
	await page.getByRole('button', { name: say('authenticator.finish'), exact: true }).click();
	await expect(page.getByRole('alert')).toHaveText(say('setup.wrong_code'));
	await page.getByLabel(say('login.code'), { exact: true }).fill(totp(fresh));
	// The window changes size just as this change of page ends, as a phone turned then would. The cross-fade waited for
	// a frame that never came: the screen stood still for 4 s, the old page under the new one (D19). It takes the
	// moment about two runs in three; on the other runs this checks nothing, and never fails.
	await page.exposeFunction('resizeNow', () => page.setViewportSize({ width: 390, height: 560 }));
	await page.evaluate(() => {
		const start = document.startViewTransition.bind(document);
		document.startViewTransition = ((update: () => Promise<void>) =>
			start(async () => {
				await update();
				document.startViewTransition = start;
				(window as unknown as { resizeNow: () => void }).resizeNow();
			})) as typeof document.startViewTransition;
	});
	await page.getByRole('button', { name: say('authenticator.finish'), exact: true }).click();
	await expect(page).toHaveURL(/\/security$/);
	const nextFrame = await page.evaluate(
		() =>
			new Promise<number>((done) => {
				const from = performance.now();
				requestAnimationFrame(() => done(performance.now() - from));
			})
	);
	expect(nextFrame).toBeLessThan(1000);
	await expect(page.getByTestId('recovery-status')).toHaveText(
		left('security.codes_left_other', 9)
	);

	// A new set, after the app's next code; the old set stops working.
	// On a short phone the form opens down to its buttons: at 390 × 600 «Crear códigos» sat 9 px under the bottom bar
	// (pass 23). 560 high here (set at the landing above), so the case holds whatever the language's line breaks.
	await expect(page.locator('main')).toHaveCount(1);
	await page.evaluate(() => scrollTo(0, 0));
	await still(page);
	await page.getByRole('button', { name: say('security.regenerate'), exact: true }).click();
	await expect(page.getByLabel(say('login.code'), { exact: true })).toBeFocused();
	const send = page.getByRole('button', { name: say('security.confirm'), exact: true });
	const under = () =>
		send.evaluate(
			(button) =>
				button.getBoundingClientRect().bottom -
				document.querySelector('nav.bar')!.getBoundingClientRect().top
		);
	await expect.poll(under).toBeLessThanOrEqual(0);
	// Cancelled and asked for again before the form has left: the form comes back and takes the code. It came back
	// with its field disabled, shut on its way out (pass 23 sweep).
	await page.evaluate(
		async ([cancel, again]) => {
			const words = (text: string) => text.replace(/\s+/g, ' ').trim();
			const button = (name: string) =>
				[...document.querySelectorAll('button')].find(
					(b) => words(b.textContent ?? '') === words(name)
				)!;
			button(cancel).click();
			await new Promise(requestAnimationFrame);
			button(again).click();
		},
		[say('security.cancel'), say('security.regenerate')]
	);
	const typed = page.getByRole('textbox', {
		name: say('login.code'),
		exact: true,
		disabled: false
	});
	await expect(typed).toBeFocused();
	await expect(page.getByLabel(say('login.code'), { exact: true })).toHaveCount(1);
	await page.setViewportSize({ width: 1280, height: 720 });
	await page.getByLabel(say('login.code'), { exact: true }).fill(totp(fresh, Date.now() + 30_000));
	// Sent from the keyboard: the new codes' title takes the focus and wears the ring, as every focused title does;
	// it had none (pass 20).
	// The form leaving keeps its height: stretched to the taller codes coming in, its buttons went 117 px in a frame (pass 58).
	await watchTop(page, '.mode form .buttons');
	await page.getByRole('button', { name: say('security.confirm'), exact: true }).press('Enter');
	const title = page.getByRole('heading', { name: say('security.new_title') });
	await expect(title).toBeFocused();
	expect(
		await title.evaluate((e) => e.matches(':focus-visible') && getComputedStyle(e).outlineStyle)
	).toBe('solid');
	// Its ring, 2 px off the words and 3 px wide, sat on the line under it, over its accents (pass 21).
	const clear = await title.evaluate((e) => {
		const style = getComputedStyle(e);
		const ring =
			e.getBoundingClientRect().bottom +
			parseFloat(style.outlineOffset) +
			parseFloat(style.outlineWidth);
		const next = document.createRange();
		next.selectNodeContents(e.nextElementSibling!);
		return next.getClientRects()[0].top - ring;
	});
	expect(clear).toBeGreaterThanOrEqual(0);
	// And 6 px off its first and last letters: 2 px hugged them (pass 23).
	const side = await title.evaluate((e) => {
		const words = document.createRange();
		words.selectNodeContents(e);
		const text = words.getBoundingClientRect();
		const box = e.getBoundingClientRect();
		const offset = parseFloat(getComputedStyle(e).outlineOffset);
		return Math.min(text.left - box.left, box.right - text.right) + offset;
	});
	expect(side).toBeGreaterThanOrEqual(6);
	await expectGlidedTop(page, 40);
	const renewed = await shownCodes(page);
	expect(renewed.some((code) => codes.includes(code))).toBe(false);
	await expect(page.getByTestId('recovery-status')).toHaveText(
		left('security.codes_left_other', 10)
	);
	// «Listo, los guardé» at the end of a short phone screen: the codes left, the card shrank at once and the browser
	// pulled the page back 165 px in that frame (pass 16 sweep). Every frame from the click on.
	await page.setViewportSize({ width: 390, height: 600 });
	await page.evaluate(() => scrollTo(0, document.documentElement.scrollHeight));
	await still(page);
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
	await page
		.getByRole('button', { name: say('security.done'), exact: true })
		.click({ force: true });
	await page.waitForFunction(() => (window as unknown as Frames).frames.length >= 150);
	const frames = await page.evaluate(() => (window as unknown as Frames).frames);
	const pulled = frames
		.slice(1)
		.filter(([y, height], i) => height < frames[i][1] && y !== frames[i][0]);
	expect(frames[0][0] - frames[frames.length - 1][0]).toBeGreaterThan(40);
	expect(pulled).toEqual([]);
	await expect(page.getByRole('button', { name: say('security.regenerate') })).toBeFocused();
	await page.setViewportSize({ width: 1280, height: 720 });

	await logOut(page);
	await logInWithRecovery(page, codes[1]);
	await expect(page.getByRole('alert')).toHaveText(say('login.recovery_wrong'));
	await logInWithRecovery(page, renewed[0]);
	await expect(page).toHaveURL(/\/authenticator$/);
});

test('a password shown goes back to dots when the form refuses to send', async ({ page }) => {
	await page.goto('/login');
	const password = page.getByLabel(say('login.password'), { exact: true });
	await password.fill('anything');
	await page.getByRole('button', { name: say('password.show') }).click();
	await expect(password).toHaveAttribute('type', 'text');
	await page.getByRole('button', { name: say('login.title') }).click();
	await expect(page.getByLabel(say('login.email'))).toHaveAttribute('aria-invalid', 'true');
	await expect(password).toHaveAttribute('type', 'password');
});

test('the login swaps its second step by a fade through, the two never at once', async ({
	page
}) => {
	await page.goto('/login');
	const swap = page.getByRole('button', { name: say('login.use_recovery'), exact: true });
	await expect(swap).toBeVisible();
	// Once the login's entrance has played (the glows run for ever and do not count).
	await page.waitForFunction(() =>
		document
			.getAnimations()
			.every((a) => a.playState !== 'running' || a.effect?.getTiming().iterations === Infinity)
	);
	// Every frame: how many of the second step's blocks show in its one cell. Crossing, the code's hint and the
	// recovery code's were drawn over each other halfway (pass 19).
	await page.evaluate(() => {
		const w = window as unknown as { shown: number[] };
		w.shown = [];
		const cell = document.querySelector('.swap')!.parentElement!;
		const step = () => {
			w.shown.push(
				[...cell.children].filter((e) => Number(getComputedStyle(e).opacity) > 0.02).length
			);
			if (w.shown.length < 60) requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	});
	await swap.click();
	await page.waitForFunction(() => (window as unknown as { shown: number[] }).shown.length >= 60);
	const shown = await page.evaluate(() => (window as unknown as { shown: number[] }).shown);
	expect(shown.filter((count) => count > 1)).toEqual([]);
	expect(shown.at(-1)).toBe(1);
	await expect(page.getByLabel(say('login.recovery_code'))).toBeFocused();
	// Back and forth again before the step has left: the recovery code comes back and takes typing. It came back
	// disabled, shut on its way out (pass 23 sweep).
	await page.evaluate(
		async ([back, again]) => {
			const words = (text: string) => text.replace(/\s+/g, ' ').trim();
			const button = (name: string) =>
				[...document.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')].find(
					(b) => words(b.textContent ?? '') === words(name)
				)!;
			button(back).click();
			await new Promise(requestAnimationFrame);
			button(again).click();
		},
		[say('login.use_code'), say('login.use_recovery')]
	);
	await expect(page.getByLabel(say('login.recovery_code'))).toBeEnabled();
	await expect(page.getByLabel(say('login.recovery_code'))).toBeFocused();
});

test('on a short phone the login keeps its top band of lines, above the brand', async ({
	page
}) => {
	// An iPhone 13 with Safari's bars shows 390 × 664: one 390 × 844 drawing centred on it put the band at −82 px,
	// off the screen (pass 20).
	await page.setViewportSize({ width: 390, height: 664 });
	await page.goto('/login');
	await expect(page.getByRole('heading', { name: say('login.title') })).toBeVisible();
	const { band, brand } = await page.evaluate(() => {
		const tops = [...document.querySelectorAll('svg.narrow path')]
			.map((path) => path.getBoundingClientRect())
			.sort((a, b) => a.top - b.top)
			.slice(0, 2);
		return {
			band: [tops[0].top, Math.max(...tops.map((r) => r.bottom))],
			brand: document.querySelector('.brand')!.getBoundingClientRect().top
		};
	});
	expect(band[0]).toBeGreaterThanOrEqual(0);
	expect(band[1]).toBeLessThanOrEqual(brand);
});
