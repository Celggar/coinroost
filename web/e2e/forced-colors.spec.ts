// Forced colors (a high-contrast theme) paint every field in one ink and drop shadows, so an error's red is gone:
// a field in error still stands apart from a valid one, on the sign-in page and in the app, and an error from a hint
// (pass 19). Its own user (see run.sh).
import { expect, test, type Locator } from '@playwright/test';
import { unfold } from './forms.ts';
import { say } from './messages.ts';
import { totp } from './totp.ts';

test.use({ locale: 'es-PA' });

/** A box's border width on one side, in px. */
const border = (box: Locator, side: 'top' | 'left' = 'top') =>
	box.evaluate(
		(node, side) => parseFloat(getComputedStyle(node).getPropertyValue(`border-${side}-width`)),
		side
	);

test('a field in error stands apart from a valid one, and its error from a hint', async ({
	page
}) => {
	const secret = process.env.E2E_CONTRAST_SECRET ?? '';
	expect(secret, 'run through e2e/run.sh').not.toBe('');
	await page.emulateMedia({ forcedColors: 'active' });
	await page.goto('/accounts');

	// The recovery link keeps the links' system colour, as «Atrás» on setup does: it came out as plain text (pass 29).
	const link = await page.evaluate(() => {
		const a = document.body.appendChild(document.createElement('a'));
		a.href = '/';
		const colour = getComputedStyle(a).color;
		a.remove();
		return colour;
	});
	const recovery = page.getByRole('button', { name: say('login.use_recovery') });
	await expect(recovery).toHaveCSS('color', link);

	// The sign-in page, sent with the password only: the email and the code are marked.
	const email = page.getByLabel(say('login.email'));
	const password = page.getByLabel(say('login.password'));
	await password.fill(process.env.E2E_CONTRAST_PASSWORD ?? '');
	await page.getByRole('button', { name: say('login.title') }).click();
	await expect(email).toHaveAttribute('aria-invalid', 'true');
	expect(await border(email)).toBeGreaterThanOrEqual(3 * (await border(password)));
	expect(await border(page.locator('.code .slot').first())).toBeGreaterThanOrEqual(3);

	await email.fill(process.env.E2E_CONTRAST_EMAIL ?? '');
	await page.getByLabel(say('login.code')).fill(totp(secret));
	await page.getByRole('button', { name: say('login.title') }).click();
	await expect(page.getByRole('heading', { name: say('accounts.title') })).toBeVisible();

	// An app form sent empty: «Nombre» is marked, «Marcadores» (optional) is not.
	await unfold(page);
	await page.getByRole('button', { name: say('accounts.create') }).click();
	const name = page.getByLabel(say('accounts.name'), { exact: true });
	const markers = page.getByLabel(say('accounts.markers'));
	await expect(name).toHaveAttribute('aria-invalid', 'true');
	await expect(markers).not.toHaveAttribute('aria-invalid', 'true');
	expect(await border(name)).toBeGreaterThanOrEqual(3 * (await border(markers)));
	const error = page.locator('.field-error').first();
	await expect(error).toBeVisible();
	expect(await border(error, 'left')).toBeGreaterThanOrEqual(3);
	expect(await border(page.locator('form .hint').first(), 'left')).toBe(0);
});
