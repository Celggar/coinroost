// The feedback panel against a real server (see run.sh): two screens captured into one report, each screenshot with its
// own note, an image from the device, sent once and read back. Its own user.
import { expect, test, type BrowserContext, type Locator, type Page } from '@playwright/test';
import { say } from './messages.ts';
import { totp } from './totp.ts';

// A TOTP code is accepted once, so the first test logs in and the second reuses its session.
test.describe.configure({ mode: 'serial' });
test.use({ locale: 'es-PA' });
let session: Awaited<ReturnType<BrowserContext['storageState']>> | undefined;

async function logIn(page: Page) {
	const secret = process.env.E2E_FEEDBACK_SECRET ?? '';
	expect(secret, 'run through e2e/run.sh').not.toBe('');
	await page.goto('/');
	await page.getByLabel(say('login.email')).fill(process.env.E2E_FEEDBACK_EMAIL ?? '');
	await page.getByLabel(say('login.password')).fill(process.env.E2E_FEEDBACK_PASSWORD ?? '');
	await page.getByLabel(say('login.code')).fill(totp(secret));
	await page.getByRole('button', { name: say('login.title') }).click();
	await expect(
		page.getByRole('heading', { name: say('dashboard.title'), exact: true })
	).toBeVisible();
}

/** Watches `panel` for the next screenshot row and records whether it was animating in its first frame (#96: nothing
 * appears in one frame; with reduced motion it fades). Read with `arrived`. */
const watchRow = (panel: Locator) =>
	panel.evaluate((node) => {
		const w = window as unknown as { rowMoved?: boolean };
		w.rowMoved = undefined;
		const observer = new MutationObserver(() => {
			const row = node.querySelector('li:last-child');
			if (!row || row.getAttribute('data-seen')) return;
			row.setAttribute('data-seen', '1');
			observer.disconnect();
			requestAnimationFrame(() => (w.rowMoved = row.getAnimations({ subtree: true }).length > 0));
		});
		node.querySelectorAll('li').forEach((li) => li.setAttribute('data-seen', '1'));
		observer.observe(node, { childList: true, subtree: true });
	});
const arrived = (page: Page) =>
	page
		.waitForFunction(() => (window as unknown as { rowMoved?: boolean }).rowMoved !== undefined)
		.then(() => page.evaluate(() => (window as unknown as { rowMoved?: boolean }).rowMoved));

/** Every thumbnail drew: the app's CSP once blocked them (they were `blob:` URLs) and they showed broken. */
const drawn = (panel: Locator) =>
	panel
		.locator('li img')
		.evaluateAll((images) =>
			images.map(
				(i) => (i as HTMLImageElement).complete && (i as HTMLImageElement).naturalWidth > 0
			)
		);

for (const reducedMotion of ['no-preference', 'reduce'] as const) {
	test(`two screens and an image make one report, each note with its screenshot (${reducedMotion})`, async ({
		browser
	}) => {
		const context = await browser.newContext({
			locale: 'es-PA',
			reducedMotion,
			storageState: session
		});
		const page = await context.newPage();
		// The capture library comes only with a capture: none of the scripts a screen loads holds it (its own
		// `data-snapdom` marks; the layout only names it in its `import()`).
		const scripts: string[] = [];
		page.on('response', (r) => r.url().includes('/_app/immutable/') && scripts.push(r.url()));
		if (session) {
			await page.goto('/');
			await expect(
				page.getByRole('heading', { name: say('dashboard.title'), exact: true })
			).toBeVisible();
		} else {
			await logIn(page);
			session = await context.storageState();
		}
		const holds = async (urls: string[]) =>
			(
				await Promise.all(
					urls.map(async (u) => (await (await page.request.get(u)).text()).includes('data-snapdom'))
				)
			).some(Boolean);
		expect(await holds([...scripts]), 'the first load carries the capture library').toBe(false);

		const open = page.getByRole('button', { name: say('feedback.button'), exact: true });
		await open.click();
		const panel = page.getByRole('dialog', { name: say('feedback.title') });
		await expect(panel).toBeVisible();
		await panel.getByLabel(say('feedback.message')).fill('El total no cuadra.');
		const loaded = scripts.length;
		await watchRow(panel);
		await panel.getByRole('button', { name: say('feedback.capture') }).click();
		expect(await arrived(page), 'the screenshot row arrives, not in one frame').toBe(true);
		const first = panel.getByLabel(say('feedback.note', { n: 1 }));
		await expect(first).toBeFocused();
		// The first test brought the library into this browser's cache; a new context fetches it again.
		expect(await holds(scripts.slice(loaded)), 'the capture brought its library').toBe(true);
		await first.fill('Aquí el total');
		await panel.getByRole('button', { name: say('app.close') }).click();
		await expect(panel).toBeHidden();
		await expect(open).toBeFocused();
		await expect(open).toHaveAccessibleDescription(say('feedback.waiting_one'));

		// Another screen, through the app's own link: the draft goes with the person.
		await page
			.getByRole('link', { name: say('nav.rules') })
			.first()
			.click();
		await page.waitForURL('**/rules');
		await open.click();
		await expect(panel.getByLabel(say('feedback.note', { n: 1 }))).toHaveValue('Aquí el total');
		await panel.getByRole('button', { name: say('feedback.capture') }).click();
		await panel.getByLabel(say('feedback.note', { n: 2 })).fill('Y aquí la regla');
		const image = await page.screenshot({ clip: { x: 0, y: 0, width: 120, height: 120 } });
		await panel
			.locator('input[type=file]')
			.setInputFiles({ name: 'banco.png', mimeType: 'image/png', buffer: image });
		await panel.getByLabel(say('feedback.note', { n: 3 })).fill('Lo que dice el banco');
		expect(await drawn(panel)).toEqual([true, true, true]);
		await panel.getByRole('button', { name: say('feedback.send'), exact: true }).click();
		await expect(panel).toBeHidden();
		await expect(page.getByText(say('feedback.sent')).last()).toBeVisible();

		const list = await (await page.request.get('/api/v1/feedback')).json();
		const report = list.items[0];
		expect(report.message).toBe('El total no cuadra.');
		expect(report.context.path).toBe('/rules');
		expect(report.shots.map((s: { note: string }) => s.note)).toEqual([
			'Aquí el total',
			'Y aquí la regla',
			'Lo que dice el banco'
		]);
		for (const shot of report.shots) {
			expect(shot.media_type).toBe('image/jpeg');
			expect(shot.bytes).toBeLessThanOrEqual(600_000);
		}
		// Sent, the draft starts over: nothing waits on the button.
		await expect(open).toHaveAccessibleDescription('');

		// The report on «Comentarios», resolved from there.
		await page.goto('/feedback');
		const card = page.getByRole('article').first();
		await expect(card.getByText('Lo que dice el banco')).toBeVisible();
		await card.getByRole('button', { name: say('feedback.resolve') }).click();
		await expect(card.getByText(say('feedback.state_resolved'))).toBeVisible();
		await context.close();
	});
}
