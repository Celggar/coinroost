// The line under the login's title, through the browser, against a real server (see run.sh): it says what the second
// step asks for, and trading places moves nothing under it.
import { expect, test, type Page } from '@playwright/test';
import { catalogs, english, say, type Key } from './messages.ts';

test.use({ locale: 'es-PA' });

const WIDTHS = [...Array.from({ length: 81 }, (_, i) => 320 + i), 600, 899, 900, 1280];

/** Once the login's entrance has played (the glows run for ever and do not count). */
async function settled(page: Page) {
	await page.waitForFunction(() =>
		document
			.getAnimations()
			.every((a) => a.playState !== 'running' || a.effect?.getTiming().iterations === Infinity)
	);
}

/** At each width, the form's distance from the title and the height of each line's text, whichever shows. The text's: the
 * cell stretches both boxes to the taller. */
async function rooms(page: Page) {
	const seen: Record<number, { form: number; lines: number[] }> = {};
	for (const width of WIDTHS) {
		await page.setViewportSize({ width, height: 900 });
		seen[width] = await page.evaluate(() => {
			const top = (selector: string) =>
				document.querySelector(selector)!.getBoundingClientRect().top;
			return {
				form: Math.round(top('form') - top('h1')),
				lines: [...document.querySelectorAll('.leads > p')].map((line) => {
					const text = document.createRange();
					text.selectNodeContents(line);
					return Math.round(text.getBoundingClientRect().height);
				})
			};
		});
	}
	return seen;
}

for (const language of ['es', 'en'] as const) {
	const words = (key: Key) => catalogs[language][key];

	test(`in ${language} the line says what the second step asks and the form stays put`, async ({
		page
	}) => {
		await page.addInitScript(
			(picked) => localStorage.setItem('coinroost.locale', picked),
			language
		);
		await page.goto('/login');
		const app = page.getByText(words('login.subtitle'), { exact: true });
		const recovery = page.getByText(words('login.recovery_subtitle'), { exact: true });
		await expect(app).toBeVisible();
		await expect(recovery).toBeHidden();
		await settled(page);
		const before = await rooms(page);
		// The recovery line never adds a line: the approved form keeps its place at every width.
		const taller = WIDTHS.filter((width) => before[width].lines[1] > before[width].lines[0]);
		expect(taller).toEqual([]);

		await page.setViewportSize({ width: 390, height: 900 });
		await page.getByRole('button', { name: words('login.use_recovery'), exact: true }).click();
		await expect(recovery).toBeVisible();
		await expect(app).toBeHidden();
		await settled(page);
		// Where the two lines wrap differently, one line in the cell moved the form a line at each switch.
		const after = await rooms(page);
		expect(WIDTHS.map((width) => after[width].form)).toEqual(
			WIDTHS.map((width) => before[width].form)
		);
	});
}

for (const reducedMotion of ['no-preference', 'reduce'] as const) {
	test(`the two lines trade places by a fade, never both (${reducedMotion})`, async ({ page }) => {
		await page.emulateMedia({ reducedMotion });
		await page.goto('/login');
		const swap = page.getByRole('button', { name: say('login.use_recovery'), exact: true });
		await expect(swap).toBeVisible();
		await settled(page);
		await page.evaluate(() => {
			const w = window as unknown as { faded: number[][] };
			w.faded = [];
			const step = () => {
				w.faded.push(
					[...document.querySelectorAll('.leads > p')].map((line) => {
						const style = getComputedStyle(line);
						return style.visibility === 'hidden' ? 0 : Number(style.opacity);
					})
				);
				if (w.faded.length < 60) requestAnimationFrame(step);
			};
			requestAnimationFrame(step);
		});
		await swap.click();
		await page.waitForFunction(
			() => (window as unknown as { faded: number[][] }).faded.length >= 60
		);
		const faded = await page.evaluate(() => (window as unknown as { faded: number[][] }).faded);
		expect(faded.filter(([app, recovery]) => app > 0.02 && recovery > 0.02)).toEqual([]);
		// A fade each way: in a frame, a line came and went at full opacity.
		const between = (n: number) => n > 0.05 && n < 0.95;
		expect(faded.filter(([app]) => between(app)).length).toBeGreaterThan(0);
		expect(faded.filter(([, recovery]) => between(recovery)).length).toBeGreaterThan(0);
		expect(faded.at(-1)).toEqual([0, 1]);
	});
}

test('pressed back and forth at once the line ends with the mode, and a new language keeps it', async ({
	page
}) => {
	await page.goto('/login');
	await expect(page.getByRole('button', { name: say('login.use_recovery') })).toBeVisible();
	await settled(page);
	await page.evaluate(
		async (names) => {
			const words = (text: string) => text.replace(/\s+/g, ' ').trim();
			for (const name of names) {
				[...document.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')]
					.find((b) => words(b.textContent ?? '') === words(name))!
					.click();
				await new Promise(requestAnimationFrame);
			}
		},
		[say('login.use_recovery'), say('login.use_code'), say('login.use_recovery')]
	);
	await expect(page.getByText(say('login.recovery_subtitle'), { exact: true })).toBeVisible();
	await expect(page.getByText(say('login.subtitle'), { exact: true })).toBeHidden();
	await page.getByRole('radio', { name: 'English' }).check();
	await expect(page.getByText(english('login.recovery_subtitle'), { exact: true })).toBeVisible();
	await expect(page.getByText(english('login.subtitle'), { exact: true })).toBeHidden();
});
