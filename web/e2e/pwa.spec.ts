// The app installs, through the browser, against a real server (see run.sh): a manifest the browser reaches behind an access
// proxy, icons that load at their sizes, a service worker that keeps the app's own files and never the API, and «Instalar la
// app» on «Más».
import { expect, test, type Browser, type BrowserContext } from '@playwright/test';
import { say } from './messages.ts';
import { totp } from './totp.ts';

test.use({ locale: 'es-PA', serviceWorkers: 'allow' });
test.describe.configure({ mode: 'serial' });

let session: Awaited<ReturnType<BrowserContext['storageState']>>;

async function logIn(browser: Browser) {
	const secret = process.env.E2E_PWA_SECRET ?? '';
	expect(secret, 'run through e2e/run.sh').not.toBe('');
	// «Más» is a phone's page: a wide window goes on to the summary.
	const page = await browser.newPage({ locale: 'es-PA', viewport: { width: 390, height: 844 } });
	await page.goto('/more');
	await page.getByLabel(say('login.email')).fill(process.env.E2E_PWA_EMAIL ?? '');
	await page.getByLabel(say('login.password')).fill(process.env.E2E_PWA_PASSWORD ?? '');
	await page.getByLabel(say('login.code')).fill(totp(secret));
	await page.getByRole('button', { name: say('login.title') }).click();
	await expect(page.getByRole('heading', { name: say('more.title') })).toBeVisible();
	session = await page.context().storageState();
	await page.close();
}

test('the manifest goes with the cookie, and its icons load at the sizes it names', async ({
	page
}) => {
	await page.goto('/login');
	const link = page.locator('link[rel="manifest"]');
	// Without it an access proxy turns the manifest away and the app cannot be installed.
	await expect(link).toHaveAttribute('crossorigin', 'use-credentials');
	const response = await page.request.get((await link.getAttribute('href'))!);
	expect(response.ok()).toBe(true);
	const manifest = await response.json();
	expect(manifest).toMatchObject({ name: 'Coinroost', start_url: '/', display: 'standalone' });
	const touch = (await page.locator('link[rel="apple-touch-icon"]').getAttribute('href'))!;
	const icons = [...manifest.icons, { src: touch, sizes: '180x180', purpose: 'any' }];
	const drawn = await page.evaluate(
		(icons: { src: string; sizes: string; purpose: string }[]) =>
			Promise.all(
				icons.map(async (icon) => {
					const bitmap = await createImageBitmap(await (await fetch(icon.src)).blob());
					return { ...icon, drawn: `${bitmap.width}x${bitmap.height}` };
				})
			),
		icons
	);
	expect(drawn.filter((icon) => icon.drawn !== icon.sizes)).toEqual([]);
	expect(drawn.map((icon) => icon.sizes)).toEqual(expect.arrayContaining(['192x192', '512x512']));
	expect(drawn.map((icon) => icon.purpose)).toContain('maskable');
});

test('the service worker keeps the app and never the API, and the app opens offline', async ({
	page,
	context
}) => {
	await page.goto('/login');
	await page.evaluate(() => navigator.serviceWorker.ready);
	// A page that loaded before the worker took over is not under it.
	await page.reload();
	await expect(page.getByRole('heading', { name: say('login.title') })).toBeVisible();
	expect(await page.evaluate(() => Boolean(navigator.serviceWorker.controller))).toBe(true);
	const kept = await page.evaluate(async () => {
		const paths: string[] = [];
		for (const key of await caches.keys())
			for (const request of await (await caches.open(key)).keys())
				paths.push(new URL(request.url).pathname);
		return paths;
	});
	expect(kept).toContain('/200.html');
	// The ledger and the session are always the server's answer.
	expect(kept.filter((path) => path.startsWith('/api/'))).toEqual([]);
	await context.setOffline(true);
	const offline = await page.goto('/rules');
	expect(offline?.status()).toBe(200);
	// The kept shell, not the browser's page for no connection, and it says why nothing loads: the shell alone stayed on
	// «Cargando…» for ever (pass 71), and the login showed the page that failed to load.
	await expect(page.locator('body > .app')).toBeAttached();
	await expect(page.getByRole('heading', { name: say('offline.title') })).toBeVisible();
	await expect(page.getByText(say('app.loading'))).toHaveCount(0);
	await page.goto('/login');
	await expect(page.getByRole('heading', { name: say('offline.title') })).toBeVisible();
	await expect(page.getByRole('heading', { name: say('error.failed_title') })).toHaveCount(0);
	// Once the network is back the app carries on by itself, to the login here (this browser has no session).
	await context.setOffline(false);
	await expect(page.getByRole('heading', { name: say('login.title') })).toBeVisible();
	await expect(page.getByRole('heading', { name: say('offline.title') })).toHaveCount(0);
});

test('«Más» offers to install where the browser hands a prompt over, and uses it once', async ({
	browser
}) => {
	await logIn(browser);
	const context = await browser.newContext({
		storageState: session,
		locale: 'es-PA',
		viewport: { width: 390, height: 844 }
	});
	const page = await context.newPage();
	await page.goto('/more');
	await expect(page.getByRole('heading', { name: say('more.title') })).toBeVisible();
	// The browser's prompt, a stand-in that counts its uses and is accepted.
	await page.evaluate(() => {
		const w = window as unknown as { prompted: number };
		w.prompted = 0;
		const prompt = Object.assign(new Event('beforeinstallprompt', { cancelable: true }), {
			prompt: async () => void (w.prompted += 1),
			userChoice: Promise.resolve({ outcome: 'accepted' })
		});
		dispatchEvent(prompt);
	});
	const install = page.getByRole('button', { name: say('install.button'), exact: true });
	await expect(install).toBeVisible();
	await install.click();
	expect(await page.evaluate(() => (window as unknown as { prompted: number }).prompted)).toBe(1);
	// Installed: nothing is offered any more.
	await expect(page.getByRole('heading', { name: say('install.title') })).toHaveCount(0);
	await context.close();
});

test('on an iPhone «Más» says how to add the app from the Share menu', async ({ browser }) => {
	const context = await browser.newContext({
		storageState: session,
		locale: 'es-PA',
		viewport: { width: 390, height: 844 },
		userAgent:
			'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1'
	});
	const page = await context.newPage();
	await page.goto('/more');
	await expect(page.getByText(say('install.share'), { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: say('install.button'), exact: true })).toHaveCount(
		0
	);
	await context.close();
});
