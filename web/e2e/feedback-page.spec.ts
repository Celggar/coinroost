// The feedback panel and «Comentarios» frame by frame, each motion setting with its own user, whose household starts with
// no report: the page's empty state, the first report and the next, resolving and a failed resolve, a failed send, the cap
// of ten, and the bar's button, which the browser must not scroll to.
import { expect, test, type Page } from '@playwright/test';
import { pickLanguage } from './account.ts';
import { english, say } from './messages.ts';
import { totp } from './totp.ts';

const users = {
	'no-preference': 'E2E_FEEDBACK_MOTION',
	reduce: 'E2E_FEEDBACK_STILL'
} as const;

async function logIn(page: Page, user: string) {
	const secret = process.env[`${user}_SECRET`] ?? '';
	expect(secret, 'run through e2e/run.sh').not.toBe('');
	await page.goto('/');
	await page.getByLabel(say('login.email')).fill(process.env[`${user}_EMAIL`] ?? '');
	await page.getByLabel(say('login.password')).fill(process.env[`${user}_PASSWORD`] ?? '');
	await page.getByLabel(say('login.code')).fill(totp(secret));
	await page.getByRole('button', { name: say('login.title') }).click();
	await expect(
		page.getByRole('heading', { name: say('dashboard.title'), exact: true })
	).toBeVisible();
}

/** Samples `probe` once a frame for `frames` frames; resolves once the first sample is in, so what follows comes after it. */
async function record(page: Page, probe: string, frames = 60) {
	await page.evaluate(
		({ probe, frames }) => {
			const w = window as unknown as { samples: unknown[] };
			const read = new Function(`return (${probe})`)() as () => unknown;
			w.samples = [];
			const step = () => {
				w.samples.push(read());
				if (w.samples.length < frames) requestAnimationFrame(step);
			};
			requestAnimationFrame(step);
		},
		{ probe, frames }
	);
	await page.waitForFunction(
		() => (window as unknown as { samples: unknown[] }).samples.length > 0
	);
}
const samples = async <T>(page: Page, frames = 60) => {
	await page.waitForFunction(
		(n) => (window as unknown as { samples: unknown[] }).samples.length >= n,
		frames
	);
	return page.evaluate(() => (window as unknown as { samples: T[] }).samples);
};

/** How much of `selector` shows, its parents' opacity with its own. */
const SHOWN = `(selector) => {
	const node = document.querySelector(selector);
	if (!node) return -1;
	let shown = 1;
	for (let e = node; e; e = e.parentElement) shown *= Number(getComputedStyle(e).opacity);
	return Math.round(shown * 100) / 100;
}`;

/** The message the panel said, and where it stands against the panel's header and the screen's foot. */
const alertPlace = (page: Page, where: string) =>
	page.evaluate((where) => {
		const said = document.querySelector(`dialog ${where} .alert`);
		const head = document.querySelector('dialog header')!.getBoundingClientRect();
		const box = said?.getBoundingClientRect();
		return box && { top: box.top, bottom: box.bottom, head: head.bottom, foot: innerHeight };
	}, where);

const wide = (page: Page) => page.screenshot({ clip: { x: 0, y: 0, width: 1280, height: 800 } });

for (const reducedMotion of ['no-preference', 'reduce'] as const) {
	test(`«Comentarios» and the panel, frame by frame (${reducedMotion})`, async ({ browser }) => {
		const context = await browser.newContext({
			locale: 'es-PA',
			reducedMotion,
			viewport: { width: 390, height: 844 }
		});
		const page = await context.newPage();
		await logIn(page, users[reducedMotion]);

		// The bar's button on a page scrolled to its middle: reached by Tab, and given the focus back as the panel closes,
		// the page stays (it went 500 to 114 at each, pass 77). The page made tall enough to scroll.
		await page.goto('/more');
		await expect(page.getByRole('heading', { name: say('more.title'), level: 1 })).toBeVisible();
		await page.evaluate(() => {
			document.querySelector<HTMLElement>('main')!.style.paddingBottom = '2000px';
			scrollTo(0, 500);
		});
		await expect.poll(() => page.evaluate(() => scrollY)).toBe(500);
		await page.evaluate(() =>
			document.querySelector<HTMLElement>('a.skip')!.focus({ preventScroll: true })
		);
		const bar = page.getByRole('button', { name: say('feedback.button_label'), exact: true });
		await record(page, '() => Math.round(scrollY)', 30);
		await page.keyboard.press('Tab');
		await expect(bar).toBeFocused();
		const tabbed = await samples<number>(page, 30);
		expect(
			tabbed.filter((y) => y !== 500),
			`Tab onto the bar button: ${tabbed.join(' ')}`
		).toEqual([]);
		// Its ring whole on the screen, inside the bar's 4 px above it (pass 77: 1.5 px of it stood off the top).
		const ring = await bar.evaluate((e) => {
			const style = getComputedStyle(e);
			const top = e.getBoundingClientRect().top;
			return top - parseFloat(style.outlineWidth) - parseFloat(style.outlineOffset);
		});
		expect(ring, 'the ring top').toBeGreaterThanOrEqual(0);
		const panel = page.getByRole('dialog', { name: say('feedback.title') });
		for (const close of ['button', 'Escape']) {
			await bar.click();
			await expect(panel).toBeVisible();
			await record(page, '() => Math.round(scrollY)', 30);
			if (close === 'button') await panel.getByRole('button', { name: say('app.close') }).click();
			else await page.keyboard.press('Escape');
			await expect(panel).toBeHidden();
			await expect(bar).toBeFocused();
			expect(new Set(await samples<number>(page, 30)), `closed by ${close}`).toEqual(
				new Set([500])
			);
		}
		await page.evaluate(
			() => (document.querySelector<HTMLElement>('main')!.style.paddingBottom = '')
		);

		// «Comentarios» with no report: its empty state gives way to the first one, sent from it, through a fade.
		await page.goto('/feedback');
		const empty = page.getByText(say('feedback.none'));
		await expect(empty).toBeVisible();
		await page
			.getByRole('button', { name: say('feedback.button_label') })
			.last()
			.click();
		await expect(panel).toBeVisible();
		await panel.getByLabel(say('feedback.message')).fill('El primero');
		await record(page, `() => [(${SHOWN})('main .empty'), (${SHOWN})('main article')]`, 60);
		await panel.getByRole('button', { name: say('feedback.send'), exact: true }).click();
		await expect(page.getByRole('article')).toHaveCount(1);
		const swap = await samples<[number, number]>(page);
		const came = swap.findIndex(([, card]) => card >= 0);
		expect(came, 'the first report came').toBeGreaterThan(0);
		expect(swap[came][1], 'its first frame').toBeLessThan(0.5);
		expect(
			swap.filter(([gone, card], i) => i && gone < 0 && swap[i - 1][0] > 0.5).length,
			'the empty state left at full opacity'
		).toBe(0);

		// Resolved and opened again: the chip and the button's words change while nothing shows; the button keeps the focus.
		const card = page.getByRole('article').first();
		const toggle = card.getByRole('button');
		for (const [state, label] of [
			['feedback.state_resolved', 'feedback.reopen'],
			['feedback.state_open', 'feedback.resolve']
		] as const) {
			await record(
				page,
				`() => { const chip = document.querySelector('main article .chip'); return [chip.textContent.trim(), (${SHOWN})('main article .chip'), (${SHOWN})('main article button span')]; }`,
				40
			);
			await toggle.click();
			await expect(card.getByText(say(state))).toBeVisible();
			await expect(toggle).toHaveText(say(label));
			await expect(toggle).toBeFocused();
			const frames = await samples<[string, number, number]>(page, 40);
			const changed = frames.findIndex(([text], i) => i && text !== frames[i - 1][0]);
			expect(changed, 'the chip changed').toBeGreaterThan(0);
			expect(frames[changed][1], 'the chip as it changed').toBeLessThan(0.05);
			expect(frames[changed][2], 'the words as they changed').toBeLessThan(0.05);
		}
		// A failed resolve says why, keeps the state and the focus.
		await page.route('**/api/v1/feedback/*', (route) =>
			route.request().method() === 'PATCH' ? route.abort() : route.continue()
		);
		await toggle.click();
		await expect(page.getByText(say('feedback.what'), { exact: false }).first()).toBeVisible();
		await expect(card.getByText(say('feedback.state_open'))).toBeVisible();
		await expect(toggle).toBeFocused();
		await page.unroute('**/api/v1/feedback/*');

		// The next report, sent from the page, opens its room at the top: the first one goes down, never up first (an
		// animate: on the cards measured the new one whole and sent them up, pass 77's fix), and not at once; with reduced
		// motion it moves only while it is faded out.
		await bar.click();
		await panel.getByLabel(say('feedback.message')).fill('El segundo');
		await record(
			page,
			`() => { const old = [...document.querySelectorAll('main article')].at(-1); return [Math.round(old.getBoundingClientRect().top), (${SHOWN})('main article:last-of-type')]; }`,
			60
		);
		await panel.getByRole('button', { name: say('feedback.send'), exact: true }).click();
		await expect(page.getByRole('article')).toHaveCount(2);
		const path = await samples<[number, number]>(page);
		const start = path[0][0];
		const moves = path.filter(([top], i) => i && top !== path[i - 1][0]);
		expect(moves.length, 'the first report moved').toBeGreaterThan(0);
		expect(Math.min(...path.map(([top]) => top)), 'it never went up').toBeGreaterThanOrEqual(start);
		if (reducedMotion === 'reduce') {
			expect(
				moves.filter(([, shown]) => shown > 0.1),
				'moved while it showed'
			).toEqual([]);
		} else {
			expect(new Set(moves.map(([top]) => top)).size, 'in a glide').toBeGreaterThan(3);
		}

		// The screenshots of a wide screen are shown whole (cut to fill a box of a phone's shape, the rail was gone).
		await page.setViewportSize({ width: 1280, height: 800 });
		await page.getByRole('button', { name: say('feedback.button'), exact: true }).click();
		await expect(panel).toBeVisible();
		const shot = { name: 'ancha.png', mimeType: 'image/png', buffer: await wide(page) };
		// One row at a time: a row on its way out still holds its label for the length of its fade.
		const removeDownTo = async (left: number) => {
			for (let n = await panel.locator('li').count(); n > left; n--) {
				await panel
					.getByRole('button', { name: say('feedback.remove_label', { n: left + 1 }) })
					.first()
					.click();
				await expect(panel.locator('li')).toHaveCount(n - 1);
			}
		};
		await panel.locator('input[type=file]').setInputFiles([shot, shot]);
		await expect(panel.locator('li')).toHaveCount(2);

		// A failed send, the panel scrolled to its end: why, under the header that holds «Enviar», in sight; the draft kept.
		for (const [width, height] of [
			[1280, 800],
			[390, 844]
		]) {
			await page.setViewportSize({ width, height });
			await page.route('**/api/v1/feedback', (route) =>
				route.request().method() === 'POST' ? route.abort() : route.continue()
			);
			await panel.evaluate((box) => (box.scrollTop = box.scrollHeight));
			const send = panel.getByRole('button', { name: say('feedback.send'), exact: true });
			await send.click();
			await expect(panel.getByRole('alert').first()).toContainText(say('feedback.what'));
			await expect(send).toBeFocused();
			await expect
				.poll(async () => {
					const at = await alertPlace(page, 'form > .said');
					return !!at && at.top >= at.head - 1 && at.bottom <= at.foot;
				}, `the reason in sight at ${width}`)
				.toBe(true);
			await expect(panel.locator('li')).toHaveCount(2);
			await page.unroute('**/api/v1/feedback');
		}

		// Ten at most: from a file that is no image and ten more, eight fit; why the rest did not is said under the add
		// buttons and comes into sight, and so does why both are off (pass 77: two dropped unsaid, the reason far above).
		for (const [width, height] of [
			[1280, 800],
			[390, 844]
		]) {
			await page.setViewportSize({ width, height });
			await removeDownTo(2);
			const many = Array.from({ length: 10 }, (_, i) => ({ ...shot, name: `m${i}.png` }));
			const text = { name: 'notas.txt', mimeType: 'text/plain', buffer: Buffer.from('hola') };
			await panel.locator('input[type=file]').setInputFiles([text, ...many]);
			await expect(panel.locator('li')).toHaveCount(10);
			const said = panel.locator('section .alert');
			await expect(said).toContainText(say('feedback.not_image', { name: 'notas.txt' }));
			await expect(said).toContainText(say('feedback.dropped_other', { count: 2 }));
			await expect
				.poll(async () => {
					const at = await alertPlace(page, 'section');
					return !!at && at.top >= at.head - 1 && at.bottom <= at.foot;
				}, `the dropped ones said in sight at ${width}`)
				.toBe(true);
			for (const name of ['feedback.capture', 'feedback.attach'] as const) {
				const add = panel.getByRole('button', { name: say(name) });
				await expect(add).toBeDisabled();
				await expect(add).toHaveAccessibleDescription(say('feedback.full'));
			}
		}
		await removeDownTo(2);
		await panel.getByRole('button', { name: say('feedback.send'), exact: true }).click();
		await expect(panel).toBeHidden();
		await page.goto('/feedback');
		await expect(page.getByRole('article').first().locator('img')).toHaveCount(2);
		const fits = await page
			.getByRole('article')
			.first()
			.locator('img')
			.evaluateAll((images) => images.map((i) => getComputedStyle(i).objectFit));
		expect(fits).toEqual(['contain', 'contain']);

		// In English at 390: every card inside the screen's width.
		await page.setViewportSize({ width: 1280, height: 800 });
		await pickLanguage(page, 'English');
		await expect(page.locator('html')).toHaveAttribute('lang', 'en');
		await page.setViewportSize({ width: 390, height: 844 });
		await expect(page.getByRole('heading', { name: english('feedback.title') })).toBeVisible();
		await expect(page.getByRole('article').first().getByRole('button')).toHaveText(
			english('feedback.resolve')
		);
		expect(
			await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth),
			'no sideways scroll'
		).toBe(true);
		await context.close();
	});
}
