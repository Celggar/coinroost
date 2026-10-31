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
