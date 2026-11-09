// Waiting for a page's motion to end before reading where it stands.
import { expect, type Page } from '@playwright/test';

/** Resolves with `scrollY` once it reads the same twice in a row, `every` ms apart: the page has stopped moving. */
export async function still(page: Page, every = 300): Promise<number> {
	let last = NaN;
	await expect
		.poll(async () => last === (last = await page.evaluate(() => scrollY)), { intervals: [every] })
		.toBe(true);
	return last;
}

/** Once every animation with an end has played: what runs for ever (the login's glows) does not count. */
export async function settled(page: Page) {
	await page.waitForFunction(() =>
		document
			.getAnimations()
			.every((a) => a.playState !== 'running' || a.effect?.getTiming().iterations === Infinity)
	);
}
