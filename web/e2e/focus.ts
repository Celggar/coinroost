// Where the focus goes during an action.
import type { Page } from '@playwright/test';

/**
 * Starts counting the times the focus falls to the page: a focusout with nowhere to go. A disabled button drops the
 * focus while it works, then the page moved it on, so a screen reader could lose its place (pass 18). Resolves with a
 * reader of what dropped it, by its text.
 */
export async function watchDrops(page: Page): Promise<() => Promise<string[]>> {
	await page.evaluate(() => {
		const w = window as unknown as { drops: string[] };
		w.drops = [];
		document.addEventListener(
			'focusout',
			(event) => {
				if (!event.relatedTarget) w.drops.push((event.target as Element).textContent?.trim() ?? '');
			},
			true
		);
	});
	return () => page.evaluate(() => (window as unknown as { drops: string[] }).drops);
}
