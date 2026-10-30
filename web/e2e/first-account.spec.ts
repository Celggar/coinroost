// The first account through the browser, against a server that allows it and has no users (see run.sh): the app
// opens on the setup page without painting the login, the same-device link fits each kind of device, and once the
// owner exists sign-up is closed and the login is back.
import { devices, expect, test, type Page } from '@playwright/test';
import { say } from './messages.ts';
import { totp } from './totp.ts';

const base = process.env.E2E_FIRST_URL ?? '';
const email = `owner-${Date.now()}@example.com`;
const password = 'a long enough password';

test.use({ locale: 'es-PA' });
test.describe.configure({ mode: 'serial' });

/** Notes whether the login heading is ever put on the page, even for one frame. */
async function watchForLogin(page: Page) {
	await page.addInitScript((title) => {
		new MutationObserver(() => {
			for (const heading of document.querySelectorAll('h1')) {
				if (heading.textContent?.trim() === title) Object.assign(window, { sawLogin: true });
			}
		}).observe(document, { childList: true, subtree: true });
	}, say('login.title'));
}

async function toCodeStep(page: Page) {
	await page.getByLabel(say('login.email')).fill(email);
	await page.getByLabel(say('login.password')).fill(password);
	await page.getByRole('button', { name: say('setup.continue') }).click();
	await expect(page.getByRole('img', { name: say('setup.qr_alt') })).toBeVisible();
}

for (const [device, scheme] of [
	['iPhone 15', 'apple-otpauth://totp/'],
	['Pixel 7', 'otpauth://totp/']
] as const) {
	test(`on ${device} the same-device link uses ${scheme}`, async ({ browser }) => {
		expect(base, 'run through e2e/run.sh').not.toBe('');
		const { defaultBrowserType: _, ...phone } = devices[device];
		const context = await browser.newContext({ ...phone, locale: 'es-PA' });
		const page = await context.newPage();
		await page.goto(`${base}/setup`);
		await toCodeStep(page);
		await expect(page.getByText(say('setup.scan_phone'))).toBeVisible();
		const link = page.getByRole('link', { name: say('setup.open_app') });
		expect(await link.getAttribute('href')).toMatch(
			new RegExp(`^${scheme.replace(/[/.]/g, '\\$&')}`)
		);
		await context.close();
	});
}

test('the first person makes the owner account, and then sign-up is closed', async ({ page }) => {
	expect(base, 'run through e2e/run.sh').not.toBe('');
	await watchForLogin(page);
	await page.goto(`${base}/`);
	await expect(page).toHaveURL(/\/setup#/);
	await expect(page.getByText(say('setup.first_lead'))).toBeVisible();
	expect(await page.evaluate(() => 'sawLogin' in window)).toBe(false);

	await toCodeStep(page);
	const qr = page.getByRole('img', { name: say('setup.qr_alt') });
	expect(await qr.getAttribute('src')).toMatch(/^data:image\/png;base64,/);
	// A desktop that is neither Apple's nor Android has nothing to open the link with.
	await expect(page.getByRole('link', { name: say('setup.open_app') })).toHaveCount(0);
	await expect(page.getByText(say('setup.scan'))).toBeVisible();
	const secret = (await page.getByTestId('setup-key').innerText()).replace(/\s/g, '');
	await page.getByLabel(say('login.code')).fill(totp(secret));
	await page.getByRole('button', { name: say('setup.finish') }).click();
	await expect(page.getByRole('button', { name: say('nav.log_out') })).toBeVisible();

	await page.getByRole('button', { name: say('nav.log_out') }).click();
	await expect(page).toHaveURL(/\/login/);
	await expect(page.getByRole('heading', { name: say('login.title') })).toBeVisible();
	await page.goto(`${base}/setup`);
	await expect(page.getByRole('heading', { name: say('setup.invalid_title') })).toBeVisible();
});
