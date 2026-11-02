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
