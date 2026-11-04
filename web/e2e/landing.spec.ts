// Where the page lands after an action that adds a row to a table, against a real server (see run.sh): no line of
// the table is left cut by the window's top. Its own user.
import { expect, test } from '@playwright/test';
import { expectPageFollows, watchRows } from './frames.ts';
import { say } from './messages.ts';
import { still } from './scroll.ts';
import { totp } from './totp.ts';

test.use({ locale: 'es-PA' });

test('an account sorted first lands with the column names whole', async ({ page }) => {
	const secret = process.env.E2E_LANDING_SECRET ?? '';
	expect(secret, 'run through e2e/run.sh').not.toBe('');
	await page.setViewportSize({ width: 1280, height: 600 });
	await page.goto('/accounts');
	await page.getByLabel(say('login.email')).fill(process.env.E2E_LANDING_EMAIL ?? '');
	await page.getByLabel(say('login.password')).fill(process.env.E2E_LANDING_PASSWORD ?? '');
	await page.getByLabel(say('login.code')).fill(totp(secret));
	await page.getByRole('button', { name: say('login.title') }).click();
	await expect(page.getByRole('heading', { name: say('accounts.title') })).toBeVisible();
	// A table taller than the screen with the form under it, so the landing has to choose where the page stands.
	const banks = ['uno', 'dos', 'tres', 'cuatro', 'cinco', 'seis', 'siete', 'ocho', 'nueve', 'diez'];
	for (const name of banks.map((bank) => `Banco ${bank}`)) {
		const data = {
			name,
			kind: 'checking',
			currency: 'USD',
			namespace: name.toLowerCase().replace(' ', '-')
		};
		expect((await page.request.post('/api/v1/accounts', { data })).ok()).toBe(true);
	}
	await page.reload();
	await page.locator('#new-account').click();
	await page.getByLabel(say('accounts.name'), { exact: true }).fill('Aaa primera');
	await page.getByRole('button', { name: say('accounts.create') }).click();
	await expect(page.getByRole('cell', { name: 'Aaa primera' })).toBeVisible();
	await still(page, 500);
	expect(await page.evaluate(() => scrollY)).toBeGreaterThan(0);
	// The column names sat across the top edge, half their height above it (pass 20).
	const cut = await page.evaluate(
		() =>
			[...document.querySelectorAll('main th, main td, main h2, main p, main label')]
				.flatMap((e) => {
					const range = document.createRange();
					range.selectNodeContents(e);
					return [...range.getClientRects()];
				})
				.filter((r) => r.height > 2 && r.top < 8 && r.bottom > 0).length
	);
	expect(cut).toBe(0);

	// With no glide the page comes to what was made through a fade: it stood there in one frame (pass 44).
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.getByLabel(say('accounts.name'), { exact: true }).fill('Aab segunda');
	await page.getByLabel(say('accounts.key')).fill('aab-segunda');
	// In view before the watch starts: the tool's own scroll to the button would count as the page's step.
	const create = page.getByRole('button', { name: say('accounts.create') });
	await create.scrollIntoViewIfNeeded();
	await still(page);
	await watchRows(page);
	await create.click();
	await expectPageFollows(page, 'a new account', false);
});
