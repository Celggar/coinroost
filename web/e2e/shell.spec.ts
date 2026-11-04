// The app's frame around every page, against a real server (see run.sh). Its own user.
import { expect, test, type BrowserContext, type Locator } from '@playwright/test';
import { say } from './messages.ts';
import { totp } from './totp.ts';

// A TOTP code is accepted once, so the first test logs in and the next reuse its session.
test.describe.configure({ mode: 'serial' });
test.use({ locale: 'es-PA' });
let session: Awaited<ReturnType<BrowserContext['storageState']>>;

test('a page loaded afresh fades in where it stands', async ({ page }) => {
	const secret = process.env.E2E_SHELL_SECRET ?? '';
	expect(secret, 'run through e2e/run.sh').not.toBe('');
	await page.goto('/rules');
	await page.getByLabel(say('login.email')).fill(process.env.E2E_SHELL_EMAIL ?? '');
	await page.getByLabel(say('login.password')).fill(process.env.E2E_SHELL_PASSWORD ?? '');
	await page.getByLabel(say('login.code')).fill(totp(secret));
	await page.getByRole('button', { name: say('login.title') }).click();
	await expect(page.getByRole('heading', { name: say('rules.title'), exact: true })).toBeVisible();
	session = await page.context().storageState();
	// Every frame from the first one: the title's top while the app shows. It came in under «Cargando…»'s 32 px box
	// and jumped up when that box left, the page already visible (pass 21).
	type Frames = { tops: number[] };
	await page.addInitScript(() => {
		const w = window as unknown as Frames;
		w.tops = [];
		const step = () => {
			const title = document.querySelector('main h1');
			const shell = document.querySelector('.shell');
			if (title && shell && Number(getComputedStyle(shell).opacity) > 0.02)
				w.tops.push(title.getBoundingClientRect().top);
			if (w.tops.length < 60) requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	});
	await page.reload();
	await page.waitForFunction(() => (window as unknown as Frames).tops.length >= 60);
	const tops = await page.evaluate(() => (window as unknown as Frames).tops);
	expect(Math.max(...tops) - Math.min(...tops)).toBeLessThan(0.5);
});

/** What sets a navigation item apart besides its colour. */
const look = (item: Locator) =>
	item.evaluate((a) => ({
		weight: Number(getComputedStyle(a).fontWeight),
		icon: getComputedStyle(a.querySelector('svg') as Element).fill
	}));

// The pill is a tint under 3:1 against the bar, so the label's weight and the icon have to say it too (N5, pass 29).
for (const width of [1280, 390]) {
	test(`at ${width} px the current page differs by more than its colour`, async ({ browser }) => {
		const context = await browser.newContext({
			storageState: session,
			locale: 'es-PA',
			viewport: { width, height: 800 }
		});
		const page = await context.newPage();
		await page.goto('/');
		const here = await look(page.locator('nav a[aria-current="page"]:visible').first());
		const other = await look(page.locator('nav a[href="/import"]:visible').first());
		expect(here.weight).toBeGreaterThan(other.weight);
		expect(here.icon).not.toBe(other.icon);
		await context.close();
	});
}

test('Tab through the bottom bar leaves the page where it stands', async ({ browser }) => {
	const context = await browser.newContext({
		storageState: session,
		locale: 'es-PA',
		viewport: { width: 390, height: 844 }
	});
	const page = await context.newPage();
	await page.goto('/rules');
	await expect(page.locator('main h1')).toBeVisible();
	// A page long enough to move, at any height of this user's data.
	await page.evaluate(() => {
		(document.querySelector('main') as HTMLElement).style.minHeight = '3000px';
		scrollTo({ top: 500, behavior: 'instant' });
	});
	const first = page.locator('nav a:visible').first();
	await first.evaluate((a: HTMLElement) => a.focus({ preventScroll: true }));
	for (let tab = 0; tab < 4; tab++) {
		await page.keyboard.press('Tab');
		expect(await page.evaluate(() => document.activeElement?.closest('nav') !== null)).toBe(true);
		await page.waitForTimeout(100);
		expect(await page.evaluate(() => scrollY)).toBe(500);
	}
	await context.close();
});
