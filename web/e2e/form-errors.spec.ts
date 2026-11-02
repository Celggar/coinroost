// Form errors in Spanish, as a person meets them: nothing is left to the browser's own bubble (which speaks the
// browser's language, outside the page), each error sits under its field in the page's language with the field
// marked and focused, and no English detail from the API reaches the page. Its own user (see run.sh).
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { say } from './messages.ts';
import { totp } from './totp.ts';

test.use({ locale: 'es-PA' });
test.describe.configure({ mode: 'serial' });

// What the API says in English; none of it may show in the Spanish page.
const ENGLISH =
	/it already exists|cannot be read|choose two different|sum to zero|unreadable date|no amount that/i;

let page: Page;

test.beforeAll(async ({ browser }) => {
	const secret = process.env.E2E_FORMS_SECRET ?? '';
	expect(secret, 'run through e2e/run.sh').not.toBe('');
	page = await browser.newPage({ locale: 'es-PA' });
	await page.goto('/accounts');
	await page.getByLabel(say('login.email')).fill(process.env.E2E_FORMS_EMAIL ?? '');
	await page.getByLabel(say('login.password')).fill(process.env.E2E_FORMS_PASSWORD ?? '');
	await page.getByLabel(say('login.code')).fill(totp(secret));
	await page.getByRole('button', { name: say('login.title') }).click();
	await expect(page.getByRole('heading', { name: say('accounts.title') })).toBeVisible();
});

test.afterAll(() => page.close());

/** The field's error is the given text, under it and tied to it, and the field is marked invalid. */
async function fieldSays(label: string, text: string) {
	const field = page.getByLabel(label, { exact: true });
	await expect(field).toHaveAttribute('aria-invalid', 'true');
	const ids = (await field.getAttribute('aria-describedby')) ?? '';
	const error = ids.split(' ').find((id) => id.endsWith('-error'));
	expect(error, `${label} names its error`).toBeTruthy();
	await expect(page.locator(`[id="${error}"]`)).toHaveText(text);
	await expect(page.locator(`[id="${error}"]`)).toBeVisible();
	return field;
}

test('an empty form says what is missing under each field, in Spanish', async () => {
	const form = page.locator('form').first();
	expect(await form.evaluate((f: HTMLFormElement) => f.noValidate)).toBe(true);
	await page.getByRole('button', { name: say('accounts.create') }).click();
	const name = await fieldSays(say('accounts.name'), say('form.required'));
	await expect(name).toBeFocused();
	// Typing clears that field's message.
	await name.fill('Corriente');
	await expect(name).not.toHaveAttribute('aria-invalid', 'true');
});

test('a key already taken is marked on «Clave», not as a loose English line', async () => {
	await page.getByLabel(say('accounts.name'), { exact: true }).fill('Corriente');
	await page.getByLabel(say('accounts.bank_file')).selectOption({ value: 'bac-account' });
	await page.getByLabel(say('accounts.key')).fill('corriente');
	await page.getByRole('button', { name: say('accounts.create') }).click();
	await expect(page.getByRole('status')).toHaveText(say('accounts.created', { name: 'Corriente' }));

	await page.getByLabel(say('accounts.name'), { exact: true }).fill('Otra');
	await page.getByLabel(say('accounts.key')).fill('corriente');
	await page.getByRole('button', { name: say('accounts.create') }).click();
	const key = await fieldSays(say('accounts.key'), say('problem.already_exists.namespace'));
	await expect(key).toBeFocused();
	await expect(page.locator('main')).not.toContainText(ENGLISH);
});

test('every wrong field is marked at once, in the words the API would use', async () => {
	// The API answers one field at a time: three wrong fields took three sends (pass 14). The form checks the API's
	// currency and key rules before sending.
	await page.getByLabel(say('accounts.name'), { exact: true }).fill('');
	await page.getByLabel(say('accounts.currency'), { exact: true }).fill('us');
	await page.getByLabel(say('accounts.key'), { exact: true }).fill('Bad Key!');
	await page.getByRole('button', { name: say('accounts.create') }).click();
	await fieldSays(say('accounts.name'), say('form.required'));
	await fieldSays(say('accounts.currency'), say('problem.currency_unknown'));
	await fieldSays(say('accounts.key'), say('problem.namespace_format'));
});

test('an amount that is not money is marked on its field before anything is sent', async () => {
	await page.goto('/rules');
	const form = page.getByRole('form', { name: say('ruleform.title') });
	await form.getByLabel(say('ruleform.name')).fill('Prueba');
	await form.getByLabel(say('ruleform.min')).fill('abc');
	await form.getByRole('button', { name: say('ruleform.add') }).click();
	const min = await fieldSays(say('ruleform.min'), say('form.amount'));
	await expect(min).toBeFocused();
});

test('rows that cannot be read say why in Spanish', async () => {
	const fixture = fileURLToPath(
		new URL('../../spec/fixtures/bac-account/input.csv', import.meta.url)
	);
	// The statement already has a date written another way (row 9); one row also loses its amounts.
	const text = readFileSync(fixture, 'latin1').replace(
		'YAPPY A JOHN DOE          , 20.00, 0.00,',
		'YAPPY A JOHN DOE          , , ,'
	);
	await page.goto('/import');
	await page.locator('input[type=file]').setInputFiles({
		name: 'estado.csv',
		mimeType: 'text/csv',
		buffer: Buffer.from(text, 'latin1')
	});
	const card = page.getByRole('article', { name: 'estado.csv' });
	const account = card.getByLabel(say('file.account'));
	if (await account.isVisible()) await account.selectOption({ label: 'Corriente' });
	await expect(card.getByText(say('row.unreadable_date', { value: '2026/05/20' }))).toBeVisible();
	await expect(card.getByText(say('row.unreadable_amount'))).toBeVisible();
	await expect(page.locator('main')).not.toContainText(ENGLISH);
});
