// The web enrollment through the browser, against a real server (see run.sh): a single-use setup link, the email
// and password, the authenticator secret the server made, a wrong code and then a right one, and the person is in;
// then out, in again with the next code, and the link no longer works.
import { expect, test } from '@playwright/test';
import { say } from './messages.ts';
import { totp } from './totp.ts';

const invite = process.env.E2E_INVITE ?? '';
const email = `web-${Date.now()}@example.com`;
const password = 'a long enough password';

test.use({ locale: 'es-PA' });

test('a setup link makes a user who can log out and in again', async ({ page }) => {
	expect(invite, 'run through e2e/run.sh').not.toBe('');
	await page.goto(`/setup#${invite}`);
	await expect(page.getByRole('heading', { name: say('setup.step_account') })).toBeVisible();
	// An email another user has is said on this step, before the code (pass 20), marked and focused.
	const field = page.getByLabel(say('login.email'));
	await field.fill(process.env.E2E_EMAIL ?? '');
	await page.getByLabel(say('login.password')).fill(password);
	await page.getByRole('button', { name: say('setup.continue') }).click();
	await expect(page.getByRole('alert')).toHaveText(say('setup.taken'));
	await expect(field).toHaveAttribute('aria-invalid', 'true');
	await expect(field).toBeFocused();
	await expect(page.getByRole('img', { name: say('setup.qr_alt') })).toHaveCount(0);
	await field.fill(email);
	await page.getByRole('button', { name: say('setup.continue') }).click();

	await expect(page.getByRole('img', { name: say('setup.qr_alt') })).toBeVisible();
	// Step 1 fades out before step 2 comes in (pass 19): its form, with its own alert region, leaves first.
	await expect(page.getByRole('heading', { name: say('setup.step_account') })).toHaveCount(0);
	const secret = (await page.getByTestId('setup-key').innerText()).replace(/\s/g, '');
	const right = totp(secret);
	const wrong = String((Number(right) + 500_000) % 1_000_000).padStart(6, '0');
	await page.getByLabel(say('login.code')).fill(wrong);
	await page.getByRole('button', { name: say('setup.finish') }).click();
	await expect(page.getByRole('alert')).toHaveText(say('setup.wrong_code'));
	// The active slot keeps the error's red inside its focus ring, like the other five (pass 20).
	const edges = await page.evaluate(() => {
		const slots = [...document.querySelectorAll('.slot')];
		const active = slots.find((slot) => slot.classList.contains('active'))!;
		const other = slots.find((slot) => slot !== active)!;
		return [
			getComputedStyle(active, '::after').borderTopColor,
			getComputedStyle(other).borderTopColor
		];
	});
	expect(edges[0]).toBe(edges[1]);
	await page.getByLabel(say('login.code')).fill(right);
	await page.getByRole('button', { name: say('setup.finish') }).click();
	// Step 3: the new user's recovery codes, shown once.
	await expect(page.getByRole('heading', { name: say('setup.step_codes') })).toBeVisible();
	await expect(
		page.getByRole('list', { name: say('recovery.list') }).getByRole('listitem')
	).toHaveCount(10);
	await page.getByRole('button', { name: say('setup.codes_done') }).click();
	await expect(page.getByRole('button', { name: say('nav.log_out') })).toBeVisible();

	await page.getByRole('button', { name: say('nav.log_out') }).click();
	await expect(page).toHaveURL(/\/login/);
	await page.getByLabel(say('login.email')).fill(email);
	await page.getByLabel(say('login.password')).fill(password);
	// The code that accepted the link is spent; the next one logs in.
	await page.getByLabel(say('login.code')).fill(totp(secret, Date.now() + 30_000));
	await page.getByRole('button', { name: say('login.title') }).click();
	await expect(page.getByRole('button', { name: say('nav.log_out') })).toBeVisible();

	await page.goto(`/setup#${invite}`);
	await expect(page.getByRole('heading', { name: say('setup.invalid_title') })).toBeVisible();
});
