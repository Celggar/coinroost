// The language follows the account, not the device (P2): a pick made before the account had one becomes the
// account's, a pick inside the app replaces it, and another browser that asks for the other language opens in the
// saved one. «Another browser» is a new context with the session cookie and no stored pick.
import { expect, test, type Browser, type BrowserContext } from '@playwright/test';
import { english, say } from './messages.ts';
import { openAccount } from './account.ts';
import { totp } from './totp.ts';

const invite = process.env.E2E_LANGUAGE_INVITE ?? '';
const email = `language-${Date.now()}@example.com`;
const password = 'a long enough password';

// The browser asks for Spanish, so picking English is a choice the app has to remember.
test.use({ locale: 'es-PA' });

/** A fresh browser asking for Spanish, logged in with the session of `from` but with nothing stored. */
async function otherDevice(browser: Browser, from: BrowserContext) {
	const { cookies } = await from.storageState();
	const context = await browser.newContext({
		locale: 'es-PA',
		storageState: { cookies, origins: [] }
	});
	const page = await context.newPage();
	await page.goto('/');
	return page;
}

const saved = (response: { url(): string; request(): { method(): string }; ok(): boolean }) =>
	response.url().endsWith('/api/v1/me') && response.request().method() === 'PATCH' && response.ok();

test('the language picked is saved on the account and wins on another browser', async ({
	browser,
	context,
	page
}) => {
	expect(invite, 'run through e2e/run.sh').not.toBe('');
	await page.goto(`/setup#${invite}`);

	// Picked before there is an account: only this browser knows it, until the account exists.
	await page.getByRole('radio', { name: 'English' }).check();
	await page.getByLabel(english('login.email')).fill(email);
	await page.getByLabel(english('login.password')).fill(password);
	await page.getByRole('button', { name: english('setup.continue'), exact: true }).click();
	const secret = (await page.getByTestId('setup-key').innerText()).replace(/\s/g, '');
	await page.getByLabel(english('login.code'), { exact: true }).fill(totp(secret));
	await page.getByRole('button', { name: english('setup.finish'), exact: true }).click();
	// Going in loads the account, which has no language yet: it takes this browser's pick.
	const first = page.waitForResponse(saved);
	await page.getByRole('button', { name: english('setup.codes_done'), exact: true }).click();
	await first;

	const second = await otherDevice(browser, context);
	await expect(second.locator('html')).toHaveAttribute('lang', 'en');
	await expect(second.getByRole('heading', { level: 1 })).toHaveText(english('dashboard.title'));

	// Picked inside the app: the account takes it at once.
	const picked = second.waitForResponse(saved);
	await openAccount(second, 'en');
	await second.getByRole('radio', { name: 'Español' }).check();
	await picked;

	const third = await otherDevice(browser, context);
	await expect(third.locator('html')).toHaveAttribute('lang', 'es');
	await expect(third.getByRole('heading', { level: 1 })).toHaveText(say('dashboard.title'));
	await second.context().close();
	await third.context().close();
});
