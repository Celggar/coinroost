// Recovery codes through the browser, against a real server (see run.sh): a setup link hands out ten codes, which
// copy and download; the Security page counts them; a login with one leads to a new authenticator and nowhere else;
// a new set retires the old one.
import { expect, test, type Page } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { say, type Key } from './messages.ts';
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
	if (!(await field.isVisible()))
		await page.getByRole('button', { name: say('login.use_recovery'), exact: true }).click();
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
	await page.getByRole('button', { name: say('nav.log_out'), exact: true }).click();
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
	await page.getByRole('button', { name: say('authenticator.finish'), exact: true }).click();
	await expect(page).toHaveURL(/\/security$/);
	await expect(page.getByTestId('recovery-status')).toHaveText(
		left('security.codes_left_other', 9)
	);

	// A new set, after the app's next code; the old set stops working.
	await page.getByRole('button', { name: say('security.regenerate'), exact: true }).click();
	await page.getByLabel(say('login.code'), { exact: true }).fill(totp(fresh, Date.now() + 30_000));
	await page.getByRole('button', { name: say('security.confirm'), exact: true }).click();
	await expect(page.getByRole('heading', { name: say('security.new_title') })).toBeVisible();
	const renewed = await shownCodes(page);
	expect(renewed.some((code) => codes.includes(code))).toBe(false);
	await expect(page.getByTestId('recovery-status')).toHaveText(
		left('security.codes_left_other', 10)
	);
	await page.getByRole('button', { name: say('security.done'), exact: true }).click();

	await page.getByRole('button', { name: say('nav.log_out'), exact: true }).click();
	await logInWithRecovery(page, codes[1]);
	await expect(page.getByRole('alert')).toHaveText(say('login.recovery_wrong'));
	await logInWithRecovery(page, renewed[0]);
	await expect(page).toHaveURL(/\/authenticator$/);
});
