// A household with nothing imported, against a real server (see run.sh): Transfers names the step that comes first,
// as Uncategorized does, instead of four cards with nothing in them (pass 21). Its own user.
import { expect, test, type Page } from '@playwright/test';
import { say } from './messages.ts';
import { totp } from './totp.ts';

test.use({ locale: 'es-PA' });

/** Transfers and Uncategorized both offer this one way on, and Transfers shows none of its cards. */
async function bothOffer(page: Page, action: string, href: string) {
	for (const path of ['/transfers', '/uncategorized']) {
		await page.goto(path);
		await expect(page.getByRole('link', { name: action })).toHaveAttribute('href', href);
	}
	await page.goto('/transfers');
	await expect(page.getByRole('link', { name: action })).toBeVisible();
	await expect(page.getByRole('heading', { name: say('waiting.title') })).toHaveCount(0);
}

test('with nothing imported, Transfers offers the first step as Uncategorized does', async ({
	page
}) => {
	const secret = process.env.E2E_EMPTY_SECRET ?? '';
	expect(secret, 'run through e2e/run.sh').not.toBe('');
	await page.goto('/transfers');
	await page.getByLabel(say('login.email')).fill(process.env.E2E_EMPTY_EMAIL ?? '');
	await page.getByLabel(say('login.password')).fill(process.env.E2E_EMPTY_PASSWORD ?? '');
	await page.getByLabel(say('login.code')).fill(totp(secret));
	await page.getByRole('button', { name: say('login.title') }).click();
	await expect(page.getByRole('heading', { name: say('transfers.title') })).toBeVisible();

	await bothOffer(page, say('dashboard.to_accounts'), '/accounts');
	// A cash account reads no bank file: the way on is still Accounts.
	const cash = { name: 'Efectivo', kind: 'cash', currency: 'USD', namespace: 'efectivo' };
	expect((await page.request.post('/api/v1/accounts', { data: cash })).ok()).toBe(true);
	await bothOffer(page, say('file.open_accounts'), '/accounts');
	// An account that reads a bank file: the way on is Import.
	const bank = {
		name: 'Banco',
		kind: 'checking',
		currency: 'USD',
		namespace: 'banco',
		profile: 'bac-account'
	};
	expect((await page.request.post('/api/v1/accounts', { data: bank })).ok()).toBe(true);
	await bothOffer(page, say('uncategorized.import_first'), '/import');
});
