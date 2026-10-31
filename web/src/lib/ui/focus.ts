import { tick } from 'svelte';

// Where the focus goes after an action that removes what had it.

/**
 * Focuses the first element, trying the selectors in order, that is staying on the page. Svelte makes an element
 * inert while it animates out, and focus() on it does nothing, so a match inside an inert subtree is skipped.
 */
export function focusOn(...selectors: string[]): boolean {
	for (const selector of selectors) {
		const target = [...document.querySelectorAll<HTMLElement>(selector)].find(
			(e) => !e.closest('[inert]')
		);
		if (target) {
			target.focus();
			return true;
		}
	}
	return false;
}

/**
 * Runs a page's load again from its «Intentar de nuevo» and puts the focus where one can go on: on the button again
 * when the load failed again, else on `heading` (the page's title by default), whose content just arrived. The button
 * leaves with its alert, and the focus fell to the page's body (pass 8).
 */
export async function retry(load: () => unknown, heading = 'main h1') {
	await load();
	await tick();
	await new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)));
	if (document.activeElement && document.activeElement !== document.body) return;
	if (focusOn('main .alert button')) return;
	const title = document.querySelector<HTMLElement>(heading);
	if (!title) return;
	if (!title.hasAttribute('tabindex')) title.tabIndex = -1;
	title.focus();
}
