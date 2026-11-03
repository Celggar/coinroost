import { tick } from 'svelte';

// Where the focus goes after an action that removes what had it.

/**
 * Focuses the first element, trying the selectors in order, that is staying on the page, and returns it. Svelte makes
 * an element inert while it animates out, and focus() on it does nothing, so a match inside an inert subtree is
 * skipped. The page does not move: the caller shows what it needs with `showWhole` once it settles (the browser's own
 * scroll jumped in one frame and left a title across the top bar, pass 13).
 */
export function focusOn(...selectors: string[]): HTMLElement | null {
	for (const selector of selectors) {
		const target = [...document.querySelectorAll<HTMLElement>(selector)].find(
			(e) => !e.closest('[inert]')
		);
		if (target) {
			target.focus({ preventScroll: true });
			return target;
		}
	}
	return null;
}

/**
 * Keeps the focus inside `box` while its rows are put in a new order, once the caller has the new order and before it
 * hands it over. A keyed list moves a row by taking it out and back in, and a focused button taken out drops the focus
 * to the page: «Bajar» dropped it every time, the row going down being the one moved (pass 20). The box takes the
 * focus in the same task, before any frame, and gives it up to whatever the caller focuses next.
 */
export function holdFocus(box: HTMLElement | null) {
	if (!box?.contains(document.activeElement)) return;
	box.tabIndex = -1;
	box.focus({ preventScroll: true });
	box.addEventListener('focusout', () => box.removeAttribute('tabindex'), { once: true });
}

/**
 * Runs a page's load again from its «Intentar de nuevo» (`from`, the button pressed) and puts the focus where one can
 * go on: on that place's button again when the load failed again, else on `heading` (the page's title by default),
 * whose content is coming back. Only the alert where the button was counts: another section's own failure keeps its
 * button (pass 9: the summary's retry went to the payroll card's and scrolled the page 331 px). The focus never waits
 * on the page's body: once the button cannot hold it (its alert left as the load started, or as it ended), it goes to
 * the title in the same moment. Waiting two frames for the body first, it sat there one frame (pass 19).
 */
export async function retry(from: EventTarget | null, load: () => unknown, heading = 'main h1') {
	// The alert's container: a failure again shows a new alert there, a new button in it.
	const place = from instanceof Element ? from.closest('.alert')?.parentElement : null;
	const gone = () =>
		!(from instanceof Element) || !from.isConnected || from.closest('[inert]') !== null;
	const toTitle = () => {
		const title = document.querySelector<HTMLElement>(heading);
		if (!title) return null;
		if (!title.hasAttribute('tabindex')) title.tabIndex = -1;
		title.focus();
		return title;
	};
	const loading = load();
	await tick();
	let held = document.activeElement === from;
	if (held && gone()) held = !!toTitle();
	await loading;
	await tick();
	// The page put the focus somewhere of its own meanwhile: it stays there.
	if (!held && document.activeElement !== document.body) return;
	const again = place?.isConnected
		? place.querySelector<HTMLElement>(':scope > .alert button:not([inert] *)')
		: null;
	if (again) {
		if (document.activeElement !== again) again.focus();
		return;
	}
	if (document.activeElement === from || document.activeElement === document.body) toTitle();
}
