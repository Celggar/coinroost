// Where the page lands after an action: the least scroll that shows what it answered inside the bars' room, smoothly,
// once the page has settled, and where no line of text is cut at the top. A browser's own scroll to a focused element
// jumped in one frame and left titles across the bar (pass 13).
import { prefersReducedMotion } from 'svelte/motion';
import { DURATION, spare } from '#lib/motion.ts';
import { fadePage } from '#lib/ui/followers.ts';

/** Pinned to the window (a bar): it does not move with the page, so the page's edge never cuts it. */
function pinned(element: Element): boolean {
	for (let a: Element | null = element; a; a = a.parentElement) {
		const position = getComputedStyle(a).position;
		if (position === 'fixed' || position === 'sticky') return true;
	}
	return false;
}

/** Where the page's content is cut at the top: under a bar pinned there, else the window's edge. The scroll padding
 * is breathing room, not a cover: on the sign-in pages nothing hides the 16 px above it. While a page's view
 * transition runs, the point hits the transition's layer, not the bar: the bar's known height stands in, or a landing
 * measured against no bar left a label across it (pass 12). */
function coverTop(): number {
	for (let a = document.elementFromPoint(innerWidth / 2, 1); a; a = a.parentElement) {
		const position = getComputedStyle(a).position;
		if (position === 'fixed' || position === 'sticky')
			return Math.max(0, a.getBoundingClientRect().bottom);
	}
	return (
		parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--bar-height')) || 0
	);
}

// A tile's label and figure are lines too: a `dt` stood 3.3 px under the bar (pass 13).
// A table's cells too: a new account sorted first landed with the column names cut by the window's top (pass 20).
// Rows that touch count by what they hold, as cells do: no line between two was clear of both, so the search went up the
// whole list, 250 to 437 px with the answer already in view (pass 42).
const LINES =
	'h1, h2, h3, p, label, legend, li:not([data-rows] > li), dt, dd, th, td, img, [data-whole]';
// A select's value stood across the bar after changes were applied (pass 14).
const CONTROLS = 'select, input, button, textarea';

/**
 * The page position nearest `y` whose cut line at the top goes through no line of text, no picture and no control:
 * up to show what it would cut whole, else down past it, within `lo`–`hi`. When only text can stay whole, text; when
 * nothing can, `y`. A heading cut in half at the top looked broken (pass 8).
 */
function uncut(
	y: number,
	lo: number,
	hi: number,
	loose = lo,
	max = document.documentElement.scrollHeight - innerHeight
): number {
	const both = `${LINES}, ${CONTROLS}`;
	// Within `lo` first, controls and then text alone, and only then down to `loose`: tried with the controls before
	// the text alone, `loose` let a whole field at the top leave the send button below the screen (pass 14).
	const strict: Ways = [
		[-1, lo],
		[1, lo]
	];
	const relaxed: Ways = [[-1, loose]];
	return (
		clean(both, strict, y, hi, max) ??
		clean(LINES, strict, y, hi, max) ??
		clean(both, relaxed, y, hi, max) ??
		clean(LINES, relaxed, y, hi, max) ??
		Math.min(Math.max(y, 0), max)
	);
}

/** Directions to search, each with the least position it may end at: -1 up, 1 down. */
type Ways = (readonly [-1 | 1, number])[];

/** `uncut` for the boxes `selector` names, searching `ways`; null when none of its places is free of them. */
function clean(selector: string, ways: Ways, y: number, hi: number, max: number): number | null {
	const line = coverTop();
	const boxes = [...document.querySelectorAll<HTMLElement>(selector)]
		.map((e) => ({ e, r: e.getBoundingClientRect() }))
		.filter(({ e, r }) => r.height > 2 && !pinned(e))
		.map(({ e, r }) => {
			// A cell counts by its words, inside its padding: the rows' boxes touch, so no line between two rows was
			// ever clear of both, and the search went up past the whole table.
			const cell = e.matches('th, td') ? getComputedStyle(e) : null;
			const pad = (side: string) =>
				cell ? parseFloat(cell.getPropertyValue(`padding-${side}`)) || 0 : 0;
			return { top: r.top + scrollY + pad('top'), bottom: r.bottom + scrollY - pad('bottom') };
		});
	// The page cannot go above its start or past its end (`max`): a position out there is no answer.
	// A box that starts within 8 px under the cover reads as cut too: a label flush with the bar's edge looked cut
	// (pass 9). Going up leaves it 8 px clear; going down hides it whole under the bar.
	const clear = 8;
	const cut = (at: number) =>
		boxes.filter((b) => b.top < at + line + clear && at + line < b.bottom);
	const start = Math.min(Math.max(y, 0), max);
	// The window first; then, when it holds no clean position, down to `loose`, which lets what must show sit in the
	// breathing room above the bottom bar: at a page's end the only clean place was 16 px up (pass 12).
	for (const [way, least] of ways) {
		let at = start;
		for (let crossing = cut(at); crossing.length; crossing = cut(at)) {
			// Whole pixels, away from the box: the browser rounds where a scroll ends, and a label put exactly 8 px
			// clear landed 7.9 px under the bar (pass 11).
			at =
				way < 0
					? Math.floor(Math.min(...crossing.map((b) => b.top)) - line - clear)
					: Math.ceil(Math.max(...crossing.map((b) => b.bottom)) - line);
			if (at < 0 || at > max) break;
		}
		// The bounds in whole pixels too, outward: hiding a card's last 0.4 px went past `hi` by that much, and the
		// landing went up a whole card instead (pass 14).
		if (
			at >= Math.max(Math.floor(least), 0) &&
			at <= Math.min(Math.ceil(hi), max) &&
			!cut(at).length
		)
			return at;
	}
	return null;
}

const behavior = (): ScrollBehavior => (prefersReducedMotion.current ? 'auto' : 'smooth');

/** With no glide the page stands elsewhere in one frame: 1039 px at full opacity on «Ver» (pass 44). It comes through a fade. */
function fadeIfFar(distance: number) {
	if (prefersReducedMotion.current && Math.abs(distance) >= 4) fadePage(DURATION.medium);
}

function go(top: number) {
	fadeIfFar(top - scrollY);
	scrollTo({ top, behavior: behavior() });
}

/** The page's top and bottom edges for content, inside the bars' room (its scroll padding). */
function edges() {
	const page = getComputedStyle(document.documentElement);
	const room = parseFloat(page.scrollPaddingTop) || 0;
	// A notice pinned at the top, as on a phone, covers what lands under it: the focus stood under one (pass 43, WCAG 2.4.11).
	const note = document.querySelector('.notices .note')?.getBoundingClientRect();
	return {
		top: note && note.top < innerHeight / 2 ? Math.max(room, note.bottom + 8) : room,
		bottom: innerHeight - (parseFloat(page.scrollPaddingBottom) || 0),
		// Where the bottom bar starts: below `bottom` lies breathing room, not a cover.
		floor: innerHeight - (parseFloat(page.getPropertyValue('--nav-height')) || 0),
		bar: parseFloat(page.getPropertyValue('--bar-height')) || 0
	};
}

/** Whether `node` shows whole between the bars, where nothing needs to move to show it. */
export function inView(node: HTMLElement): boolean {
	const { top, floor } = edges();
	const box = node.getBoundingClientRect();
	return box.top >= top && box.bottom <= floor;
}

/** Where a field's context starts: its label when it has one, else the field. */
function labelTop(control: HTMLElement): number {
	const label = 'labels' in control ? (control as HTMLInputElement).labels?.[0] : undefined;
	return Math.min(
		control.getBoundingClientRect().top,
		label?.getBoundingClientRect().top ?? Infinity
	);
}

/**
 * Focuses a field and shows it in context: its form from the title, when the title down to the field fits on screen;
 * else the field with its label. Scrolled to by itself, a field landed on the screen's edge with its label and its
 * card's title cut (pass 6). Nothing moves when that much already shows; the bars' room is the page's scroll padding.
 */
export function focusShown(
	control: HTMLElement,
	form: HTMLElement = control.closest('form') ?? control,
	take = true
): boolean {
	if (take) control.focus({ preventScroll: true });
	const { top, bottom, floor, bar } = edges();
	const field = control.getBoundingClientRect();
	const title = (form.querySelector('h2, h3, legend') ?? form).getBoundingClientRect();
	const start = form.getBoundingClientRect().top;
	// The form's send button too, when the whole form fits: landing on a field at the screen's foot left «Crear
	// categoría» below it (pass 8).
	const send = form.querySelector('button[type="submit"]')?.getBoundingClientRect();
	const end =
		send && send.bottom - start <= bottom - top
			? Math.max(send.bottom, field.bottom)
			: field.bottom;
	if (end - start <= bottom - top) {
		// The title above the screen or under its bar, or the field or the button below the screen.
		if (title.top < bar || end > bottom) {
			const y = scrollY + start - top;
			const lo = Math.max(0, scrollY + end - bottom);
			const hi = scrollY + labelTop(control) - top;
			const loose = Math.max(0, scrollY + end - floor);
			go(uncut(y, lo, hi, loose));
			return true;
		}
	} else if (labelTop(control) < top || field.bottom > bottom) {
		fadeIfFar(innerHeight);
		control.scrollIntoView({ block: 'center', behavior: behavior() });
		return true;
	}
	return false;
}

/**
 * After a send that changed the page around its form: once the page settles, the focus goes back to `control`, ready
 * for the next, and the page shows what the send `made` (a new rule in its place in the list) down to the form's button
 * or, failing that, its field, when that fits the room; else the form shown from its title, or, when that much shows
 * already, no line cut at the top. What it made too far above to show with the form (a new account sorted first, on a
 * phone) shows alone and takes the focus: left on the form, it stood 1342 px above the screen (pass 16). Measured while
 * the new rule still rose, the landing left its card 2 px under the bar, and a second landing just before it went
 * another way (pass 14). The focus moves only then: given at once, it sat on a field the person had scrolled past,
 * under the top bar, while the page settled (pass 16). `left` is where the send left it; a caller that waits before
 * landing takes it first.
 */
export async function landOn(
	control: HTMLElement,
	form: HTMLElement = control.closest('form') ?? control,
	made?: HTMLElement,
	left: Element | null = document.activeElement
) {
	await settled();
	// The person moved on while the page settled (a field filled, the form sent again and its error focused): the
	// focus and the page stay where that left them. A late landing took the focus back from «Clave» (pass 15 gates).
	if (document.activeElement !== left) return;
	if (made?.isConnected) {
		const { top, bottom } = edges();
		// Down to the form's send button when that fits too, as `focusShown` does: down to the field only, the button
		// stood below the screen at 1280.
		const from = made.getBoundingClientRect().top;
		const send = form.querySelector<HTMLElement>('button[type="submit"]');
		const end = [send, control].find(
			(e) => e && e.getBoundingClientRect().bottom - from <= bottom - top
		);
		if (!end && !made.hasAttribute('tabindex')) made.tabIndex = -1;
		await showWhole(made, end ?? made);
		await focusOnArrival(end ? control : made, left);
		return;
	}
	if (focusShown(control, form, false)) {
		await focusOnArrival(control, left);
	} else {
		control.focus({ preventScroll: true });
		await keepWhole();
	}
}

/**
 * Gives `target` the focus once some of it shows between the bars, or once the page has stopped: given as the page began to
 * glide, the name field took it 113 px above the screen (pass 17). Until then the focus stays where the send left it
 * (`left`), and a person who moves it meanwhile keeps their own.
 */
async function focusOnArrival(target: HTMLElement, left: Element | null) {
	const start = scrollY;
	let moved = false;
	for (let frames = 0, last = scrollY; frames < 90; frames++) {
		if (document.activeElement !== left) return;
		const box = target.getBoundingClientRect();
		// Not wholly hidden by the bars, as WCAG 2.4.11 asks of a focused element.
		if (box.bottom > coverTop() && box.top < edges().floor) break;
		// Stopped: a glide that ended, or none that began within a few frames.
		if ((moved && scrollY === last) || (!moved && frames > 6)) break;
		moved ||= scrollY !== start;
		last = scrollY;
		await new Promise((done) => requestAnimationFrame(done));
	}
	if (document.activeElement === left) target.focus({ preventScroll: true });
}

/** Resolves once the page stops scrolling (a smooth scroll started just before is still on its way). */
async function still() {
	for (let last = NaN, frames = 0; frames < 60; frames++) {
		await new Promise((done) => requestAnimationFrame(done));
		if (scrollY === last) return;
		last = scrollY;
	}
}

/**
 * Moves the page down just enough to show `end` (a form's send button under its message), without leaving the top edge
 * through a line of text or a picture: what it would cut goes out whole. `keep`, the field with the focus, stays on
 * screen with its label whatever is cut.
 */
export async function showBelow(end: HTMLElement, keep: HTMLElement) {
	await still();
	const { top, bottom, floor } = edges();
	const lo = scrollY + end.getBoundingClientRect().bottom - bottom;
	if (lo <= scrollY) return;
	const hi = Math.max(lo, scrollY + labelTop(keep) - top);
	// At a page's end the only uncut place may leave `end` in the breathing room above the bottom bar (pass 12:
	// after «Ver cambios» a label stood across the top bar's edge).
	const loose = Math.max(0, scrollY + end.getBoundingClientRect().bottom - floor);
	go(uncut(lo, lo, hi, loose));
}

/**
 * After content above came in under a still page (a new rule rising into the list over the form): the least scroll
 * that leaves no text cut at the top, the focused control kept on screen (pass 9: the new rule's title stood cut).
 */
export async function keepWhole() {
	await still();
	const { top, bottom } = edges();
	const active = document.activeElement;
	const focused =
		active instanceof HTMLElement && active !== document.body
			? active.getBoundingClientRect()
			: null;
	const lo = focused ? scrollY + focused.bottom - bottom : 0;
	const hi = focused ? scrollY + focused.top - top : Infinity;
	const y = uncut(scrollY, lo, hi);
	if (Math.abs(y - scrollY) >= 1) go(y);
}

/** Resolves once the page's content stands still: its entrances, exits and glides done and no scroll on its way. */
async function settled() {
	// In waves: what ends can start more (a block that waits for another's fade, a card that shrinks once it left).
	// Waiting for the first wave only, the page landed while «Parejas por confirmar» still shrank, and the page's end
	// then pulled it back over a cut line (pass 16 sweep).
	for (let wave = 0; wave < 8; wave++) {
		await new Promise((done) => requestAnimationFrame(done));
		const running = (document.querySelector('main')?.getAnimations({ subtree: true }) ?? []).filter(
			(a) => a.effect?.getTiming().iterations !== Infinity
		);
		if (!running.length) break;
		await Promise.all(running.map((a) => a.finished.catch(() => undefined)));
	}
	await still();
}

/**
 * Once the page settles, moves it the least that shows `box`, down to `end`, inside the bars' room (as
 * `scrollIntoView` with `nearest` would), smoothly and where no line of text is cut at the top; when it shows already,
 * only as far as a cut line asks. The browser's own scroll to a focused element jumped in one frame and left a title
 * across the top bar, and a list that left at the page's end pulled the page back under it (pass 13: a rule moved up,
 * a pair confirmed). `held`, a box holding room it will give back (`followContent`): below the box (or around it) it
 * ends the page that much higher, so the place is chosen inside that end; above it, the browser's scroll anchoring
 * keeps the page still as it lets go. Either way, letting it go after moves nothing on screen. Let go first, at the page's end,
 * the page slid back with it and then moved again past a cut line (pass 14: changes applied on a phone).
 */
export async function showWhole(box: HTMLElement, end: HTMLElement = box, held?: HTMLElement) {
	await settled();
	if (!box.isConnected) return;
	// An end that left while the page settled holds nothing: its box reads as 0, above everything.
	if (!end.isConnected) end = box;
	const { top, bottom, floor } = edges();
	const head = box.getBoundingClientRect();
	const foot = end.getBoundingClientRect().bottom;
	// Up to `hi` the box's top is below the top bar's room; from `lo` all of it, down to `end`, is above the bottom
	// bar's, or as much as the page's end lets show. When no such place is free of cut lines, or it is taller than
	// the room, it may go up as far as the box alone still shows (`loose`): with only its top allowed, or a place past
	// the page's end, none was free (pass 14).
	const below =
		held &&
		(held.contains(box) || box.compareDocumentPosition(held) & Node.DOCUMENT_POSITION_FOLLOWING);
	const max = document.documentElement.scrollHeight - innerHeight - (below ? spare(held) : 0);
	const hi = scrollY + head.top - top;
	const lo = Math.min(scrollY + foot - bottom, hi, max);
	const y = Math.min(Math.max(scrollY, lo), hi);
	const loose = Math.min(lo, Math.max(0, scrollY + head.bottom - floor));
	const at = uncut(y, lo, hi, loose, max);
	if (Math.abs(at - scrollY) >= 1) {
		go(at);
		if (held) await still();
	}
}
