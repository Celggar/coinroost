// The app's frame around every page, against a real server (see run.sh). Its own user.
import { expect, test } from '@playwright/test';
import { say } from './messages.ts';
import { totp } from './totp.ts';

test.use({ locale: 'es-PA' });

test('a page loaded afresh fades in where it stands', async ({ page }) => {
	const secret = process.env.E2E_SHELL_SECRET ?? '';
	expect(secret, 'run through e2e/run.sh').not.toBe('');
	await page.goto('/rules');
	await page.getByLabel(say('login.email')).fill(process.env.E2E_SHELL_EMAIL ?? '');
	await page.getByLabel(say('login.password')).fill(process.env.E2E_SHELL_PASSWORD ?? '');
	await page.getByLabel(say('login.code')).fill(totp(secret));
	await page.getByRole('button', { name: say('login.title') }).click();
	await expect(page.getByRole('heading', { name: say('rules.title'), exact: true })).toBeVisible();
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
