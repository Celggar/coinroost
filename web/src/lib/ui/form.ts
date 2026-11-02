import { prefersReducedMotion } from 'svelte/motion';
// Form checks in the page's language (FD5). Forms set `novalidate`, so the browser never shows its own bubble (in the
// browser's language, outside the page); these read the same constraints (`required`, `maxlength`, `type`,
// `pattern`, `min`/`max`) and give each invalid field its message, keyed by the field's `name`.
import { has, i18n, t } from '#lib/i18n.svelte.ts';
import { spare } from '#lib/motion.ts';

export type Errors = Record<string, string>;

type Control = HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement;

function message(control: Control): string {
	const validity = control.validity;
	// Only spaces is empty too: the API trims and would refuse it.
	if (validity.valueMissing || (control.required && control.value.trim() === '')) {
		return t(control instanceof HTMLSelectElement ? 'form.choose' : 'form.required');
	}
	if (control instanceof HTMLSelectElement) return t('form.choose');
	if (validity.tooLong) return t('form.too_long', { max: control.maxLength });
	if (validity.tooShort) return t('form.too_short', { min: control.minLength });
	if (validity.typeMismatch && control.type === 'email') return t('form.email');
	if (validity.rangeUnderflow) return t('form.min', { min: (control as HTMLInputElement).min });
	if (validity.rangeOverflow) return t('form.max', { max: (control as HTMLInputElement).max });
	if (validity.customError) return control.validationMessage;
	// A field that mirrors an API rule names the API's own words for it (`data-format`), so both say the same.
	const format = control.dataset.format;
	if (validity.patternMismatch && format && has(format)) return t(format);
	if (control.inputMode === 'decimal') return t('form.amount');
	return t('form.format');
}

/** The form's own fields that can be checked, each with the key its error goes under: its name, else its id. */
function controls(form: HTMLFormElement): [string, Control][] {
	return [...form.elements]
		.filter((e): e is Control => 'validity' in e && (e as Control).willValidate)
		.map((control) => [control.name || control.id, control] as [string, Control])
		.filter(([key]) => key !== '');
}

/** The form's invalid fields with their messages; empty when it can be sent. */
export function check(form: HTMLFormElement): Errors {
	const errors: Errors = {};
	for (const [key, control] of controls(form)) {
		const blank = control.required && control.value.trim() === '';
		if ((control.validity.valid && !blank) || errors[key]) continue;
		errors[key] = message(control);
	}
	return errors;
}

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
const LINES = 'h1, h2, h3, p, label, legend, li, dt, dd, img, [data-whole]';
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
	return (
		clean(`${LINES}, ${CONTROLS}`, y, lo, hi, loose, max) ??
		clean(LINES, y, lo, hi, loose, max) ??
		Math.min(Math.max(y, 0), max)
	);
}

/** `uncut` for the boxes `selector` names; null when none of its places is free of them. */
function clean(
	selector: string,
	y: number,
	lo: number,
	hi: number,
	loose: number,
	max: number
): number | null {
	const line = coverTop();
	const boxes = [...document.querySelectorAll<HTMLElement>(selector)]
		.map((e) => ({ e, r: e.getBoundingClientRect() }))
		.filter(({ e, r }) => r.height > 2 && !pinned(e))
		.map(({ r }) => ({ top: r.top + scrollY, bottom: r.bottom + scrollY }));
	// The page cannot go above its start or past its end (`max`): a position out there is no answer.
	// A box that starts within 8 px under the cover reads as cut too: a label flush with the bar's edge looked cut
	// (pass 9). Going up leaves it 8 px clear; going down hides it whole under the bar.
	const clear = 8;
	const cut = (at: number) =>
		boxes.filter((b) => b.top < at + line + clear && at + line < b.bottom);
	const start = Math.min(Math.max(y, 0), max);
	// The window first; then, when it holds no clean position, down to `loose`, which lets what must show sit in the
	// breathing room above the bottom bar: at a page's end the only clean place was 16 px up (pass 12).
	for (const [way, least] of [
		[-1, lo],
		[1, lo],
		[-1, loose]
	] as const) {
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

/** The page's top and bottom edges for content, inside the bars' room (its scroll padding). */
function edges() {
	const page = getComputedStyle(document.documentElement);
	return {
		top: parseFloat(page.scrollPaddingTop) || 0,
		bottom: innerHeight - (parseFloat(page.scrollPaddingBottom) || 0),
		// Where the bottom bar starts: below `bottom` lies breathing room, not a cover.
		floor: innerHeight - (parseFloat(page.getPropertyValue('--nav-height')) || 0),
		bar: parseFloat(page.getPropertyValue('--bar-height')) || 0
	};
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
	form: HTMLElement = control.closest('form') ?? control
): boolean {
	control.focus({ preventScroll: true });
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
			scrollTo({ top: uncut(y, lo, hi, loose), behavior: behavior() });
			return true;
		}
	} else if (labelTop(control) < top || field.bottom > bottom) {
		control.scrollIntoView({ block: 'center', behavior: behavior() });
		return true;
	}
	return false;
}

/**
 * After a send that changed the page around its form: once the page settles, what the send `made` (a new rule in its
 * place in the list) down to the form's button or, failing that, its field, when that fits the room; else the form shown from its title, or, when that
 * much shows already, no line cut at the top. Measured while the new rule still rose, the landing left its card 2 px
 * under the bar, and a second landing just before it went another way (pass 14).
 */
export async function landOn(
	control: HTMLElement,
	form: HTMLElement = control.closest('form') ?? control,
	made?: HTMLElement
) {
	await settled();
	const { top, bottom } = edges();
	// Down to the form's send button when that fits too, as `focusShown` does: down to the field only, the button
	// stood below the screen at 1280.
	const from = made?.isConnected ? made.getBoundingClientRect().top : NaN;
	const send = form.querySelector<HTMLElement>('button[type="submit"]');
	const end = [send, control].find(
		(e) => e && e.getBoundingClientRect().bottom - from <= bottom - top
	);
	if (made && end) {
		await showWhole(made, end);
		return;
	}
	if (!focusShown(control, form)) await keepWhole();
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
	scrollTo({ top: uncut(lo, lo, hi, loose), behavior: behavior() });
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
	if (Math.abs(y - scrollY) >= 1) scrollTo({ top: y, behavior: behavior() });
}

/** Resolves once the page's content stands still: its entrances, exits and glides done and no scroll on its way. */
async function settled() {
	await new Promise((done) => requestAnimationFrame(done));
	const running = document.querySelector('main')?.getAnimations({ subtree: true }) ?? [];
	await Promise.all(
		running
			.filter((a) => a.effect?.getTiming().iterations !== Infinity)
			.map((a) => a.finished.catch(() => undefined))
	);
	await new Promise((done) => requestAnimationFrame(done));
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
		scrollTo({ top: at, behavior: behavior() });
		if (held) await still();
	}
}

/** Moves the focus to the first field, in the form's order, that has an error. */
export function focusFirst(form: HTMLFormElement, errors: Errors) {
	const control = controls(form).find(([key]) => errors[key])?.[1];
	if (control) focusShown(control, form);
}

/**
 * For forms that show one message (the sign-in pages): marks the invalid fields with `aria-invalid` and names every
 * one of them, so no field is marked without the message saying why. Fields with the same problem share a sentence
 * («Correo y contraseña: completa estos campos.»); '' when there is none.
 */
export function single(form: HTMLFormElement, errors: Errors): string {
	const named = new Map<string, string[]>();
	const bare: string[] = [];
	for (const [key, control] of controls(form)) {
		if (!errors[key]) {
			control.removeAttribute('aria-invalid');
			continue;
		}
		control.setAttribute('aria-invalid', 'true');
		const label = control.labels?.[0]?.firstChild?.textContent?.trim();
		if (label) named.set(errors[key], [...(named.get(errors[key]) ?? []), label]);
		else bare.push(errors[key]);
	}
	const list = new Intl.ListFormat(i18n.locale, { type: 'conjunction' });
	// After the colon the sentence goes on in lower case, and a field named after the first is a word inside it
	// (RAE, «dos puntos»; Chicago 6.63): «Correo y Contraseña: Completa…» read as a heading (pass 11).
	const sentences = [...named].map(([message, fields]) =>
		t('form.named', {
			field: list.format(fields.map((field, i) => (i ? inSentence(field) : field))),
			message: inSentence(
				fields.length > 1 && message === t('form.required') ? t('form.required_many') : message
			)
		})
	);
	return [...sentences, ...bare].join(' ');
}

/** A capitalised word in lower case, to go on inside a sentence; an acronym («TOTP») keeps its capitals. */
function inSentence(text: string): string {
	return /^\p{Lu}\p{Ll}/u.test(text)
		? text[0].toLocaleLowerCase(i18n.locale) + text.slice(1)
		: text;
}

/** The errors without the one of the field being edited: a message goes as soon as its field changes. */
/** The errors without the one under `name`: for a field the page fills by itself, which sends no event. */
export function drop(errors: Errors, name: string): Errors {
	if (!(name in errors)) return errors;
	const rest = { ...errors };
	delete rest[name];
	return rest;
}

export function without(errors: Errors, event: Event): Errors {
	const name = (event.target as Control | null)?.name;
	if (!name || !(name in errors)) return errors;
	const rest = { ...errors };
	delete rest[name];
	return rest;
}
