// Transitions for things that appear and disappear. They follow the Material 3 motion tokens (in app.css): entering
// decelerates, leaving accelerates and is faster. With the reduced-motion setting on, things still fade but nothing
// moves or grows (MDN: replace the motion, do not only remove it). Svelte runs these with the Web Animations API,
// which CSS media rules do not reach, so each one reads the setting itself.
import { tick } from 'svelte';
import { prefersReducedMotion } from 'svelte/motion';
import type { AnimationConfig } from 'svelte/animate';
import { fade, fly, scale, type TransitionConfig } from 'svelte/transition';
import { fadeToPlace, fadeWhatFollows, goneBehindPage } from '#lib/ui/followers.ts';

/** Milliseconds, as in app.css. */
export const DURATION = { short: 150, exit: 200, medium: 300, long: 500 } as const;

/** The same curves as the `--ease-*` tokens in app.css, for the Web Animations API. */
export const EASE_CSS = {
	standard: 'cubic-bezier(0.2, 0, 0, 1)',
	enter: 'cubic-bezier(0.05, 0.7, 0.1, 1)',
	exit: 'cubic-bezier(0.3, 0, 0.8, 0.15)'
} as const;

/**
 * A CSS `cubic-bezier()` as a function of progress. Svelte samples the easing it is given into keyframes, so this
 * is how its transitions follow the tokens.
 */
export function bezier(x1: number, y1: number, x2: number, y2: number) {
	const curve = (a: number, b: number) => (t: number) =>
		3 * a * t * (1 - t) ** 2 + 3 * b * t ** 2 * (1 - t) + t ** 3;
	const x = curve(x1, x2);
	const y = curve(y1, y2);
	const slope = (t: number) =>
		3 * x1 * (1 - t) ** 2 + 6 * (x2 - x1) * t * (1 - t) + 3 * (1 - x2) * t ** 2;
	return (progress: number) => {
		if (progress <= 0) return 0;
		if (progress >= 1) return 1;
		let t = progress;
		for (let i = 0; i < 8; i++) {
			const error = x(t) - progress;
			if (Math.abs(error) < 1e-7) return y(t);
			const d = slope(t);
			if (Math.abs(d) < 1e-6) break;
			t = Math.min(1, Math.max(0, t - error / d));
		}
		// Newton stalls where the curve is flat; halving always lands.
		let low = 0;
		let high = 1;
		t = progress;
		while (high - low > 1e-7) {
			if (x(t) < progress) low = t;
			else high = t;
			t = (low + high) / 2;
		}
		return y(t);
	};
}

export const EASE = {
	standard: bezier(0.2, 0, 0, 1),
	enter: bezier(0.05, 0.7, 0.1, 1),
	exit: bezier(0.3, 0, 0.8, 0.15)
} as const;

const still = () => prefersReducedMotion.current;

/**
 * How long what replaces a leaving block waits: its fade (`vanish`, short) and three frames to spare. Waiting exactly
 * the fade, a fade that started two frames late still showed at 30 % when the next card moved under it (pass 16 gates).
 */
export const FADED = DURATION.exit;

/** In: rises a little into place. A delay that waits for the card to grow is dropped when nothing grows: `wait` is its own. */
export function rise(node: Element, { delay = 0, y = 12, wait = 0 } = {}): TransitionConfig {
	return fly(node, {
		delay: still() ? wait : delay,
		y: still() ? 0 : y,
		duration: DURATION.medium,
		easing: EASE.enter
	});
}

/**
 * In, to an `.in-place` cell another block is leaving: it rises once that one has faded (a fade through, the two never
 * at once). Rising while the other faded, «Nada por confirmar» was drawn over the new pairs (pass 16). With reduced
 * motion it only fades, after the same wait.
 */
export function swapIn(node: Element, { y = 12 } = {}): TransitionConfig {
	const leaving = [...(node.parentElement?.children ?? [])].some((e) => e !== node);
	return fly(node, {
		delay: leaving ? FADED : 0,
		y: still() ? 0 : y,
		duration: DURATION.medium,
		easing: EASE.enter
	});
}

/**
 * In, to a list with something under it: the item opens its own room as it fades in (its height, padding, borders and
 * the list's gap together) instead of taking it at once: «Regla nueva» moved 222 px in one frame as a new rule came in
 * above it (pass 17). The gap comes on the side that has a neighbour, held back by a negative margin. With reduced motion
 * it only fades, in its whole room at once.
 */
export function grow(node: Element): TransitionConfig {
	if (!(node instanceof HTMLElement) || still()) {
		return fade(node, { duration: DURATION.medium, easing: EASE.enter });
	}
	const style = getComputedStyle(node);
	const [height, top, bottom, edgeTop, edgeBottom] = [
		'height',
		'padding-top',
		'padding-bottom',
		'border-top-width',
		'border-bottom-width'
	].map((name) => parseFloat(style.getPropertyValue(name)) || 0);
	const list = node.parentElement;
	const gap = list ? parseFloat(getComputedStyle(list).rowGap) || 0 : 0;
	const side = node.previousElementSibling
		? 'margin-top'
		: node.nextElementSibling
			? 'margin-bottom'
			: '';
	return {
		duration: DURATION.medium,
		easing: EASE.standard,
		css: (t) =>
			`overflow: hidden; opacity: ${t}; min-height: 0; height: ${t * height}px; padding-top: ${t * top}px;` +
			` padding-bottom: ${t * bottom}px; border-top-width: ${t * edgeTop}px;` +
			` border-bottom-width: ${t * edgeBottom}px;${side ? ` ${side}: ${(t - 1) * gap}px;` : ''}`
	};
}

/** Out: lifts away, faster than it came. */
export function lift(node: Element, { y = -8 } = {}): TransitionConfig {
	return fly(node, { y: still() ? 0 : y, duration: DURATION.exit, easing: EASE.exit });
}

/** In: grows from a little smaller. */
export function pop(
	node: Element,
	{ delay = 0, start = 0.6, duration = DURATION.medium as number } = {}
): TransitionConfig {
	return scale(node, {
		delay,
		start: still() ? 1 : start,
		duration,
		easing: EASE.enter
	});
}

/**
 * A list item slides to its new place when others come or go; under reduced motion it fades there (`fadeToPlace`). One
 * moved up by an item leaving above it waits in its place while that one fades: the leaving one is taken out of the flow
 * (Svelte fixes it `absolute`), and the one moved up at once sat under it, both cards' text on top of each other (pass
 * 16; sliding up at once, it passed under the fading card). It moves by the distance between its two places alone:
 * Svelte's `flip` scales that by the box without its borders, and a bordered card waited 2 px above its place (pass 21).
 */
export function glide(node: Element, positions: { from: DOMRect; to: DOMRect }): AnimationConfig {
	const up = positions.from.top - positions.to.top;
	const side = positions.from.left - positions.to.left;
	const leaving = [...(node.parentElement?.children ?? [])].some(
		(e) => e !== node && e instanceof HTMLElement && e.style.position === 'absolute'
	);
	const wait = (up > 0 || side > 0) && leaving ? FADED : 0;
	if (still()) return fadeToPlace(up, leaving ? FADED : 0, DURATION.medium, side);
	return {
		delay: wait,
		duration: DURATION.medium,
		easing: EASE.standard,
		css: (_t, u) => `transform: translate(${u * side}px, ${u * up}px)`
	};
}

/** Out: a quick fade. */
export function vanish(node: Element): TransitionConfig {
	return fade(node, { duration: DURATION.short, easing: EASE.exit });
}

/**
 * Out of what stands in for a page's content until it loads (the «Cargando…» line, the alert of a failed load): it
 * leaves the flow at once, so the content takes its place without a jump, and fades out where it was. Only one that
 * was on screen makes `arrive` wait; the loading line stays invisible its first 300 ms.
 */
export function placeholder(node: HTMLElement): TransitionConfig {
	const shown = Number(getComputedStyle(node).opacity) > 0;
	node.style.width = `${node.getBoundingClientRect().width}px`;
	node.style.position = 'absolute';
	if (shown) node.dataset.leaving = '';
	return fade(node, { duration: DURATION.short, easing: EASE.exit });
}

/**
 * Into the content that replaces a placeholder: it fades in once the placeholder has faded out (a fade through, the
 * two never crossing), at once when none was showing. It appeared in one frame (pass 10).
 */
export function arrive(node: Element): TransitionConfig {
	const waiting = [...document.querySelectorAll('[data-leaving]')].some((e) => e.isConnected);
	return fade(node, {
		duration: DURATION.medium,
		easing: EASE.enter,
		delay: waiting ? DURATION.short : 0
	});
}

/**
 * Into a step: once the step it replaces has faded out, it slides in from the side it comes from (`direction` 1
 * forward, -1 back), as in Material 3's shared axis, the two never at once. Crossing, «Paso 1 de 3» was drawn over
 * «Paso 2 de 3» halfway (pass 19). With reduced motion it only fades, after the same wait.
 */
export function stepIn(node: Element, { direction = 1 } = {}): TransitionConfig {
	const leaving = [...(node.parentElement?.children ?? [])].some((e) => e !== node);
	return fly(node, {
		delay: leaving ? FADED : 0,
		x: still() ? 0 : 32 * direction,
		duration: DURATION.medium,
		easing: EASE.enter
	});
}

/** Shuts an element on its way out of a `{#key}` block: no focus or clicks, and its fields drop out of the form, or an
 * empty required one left there for the length of the exit would block a quick submit of what replaced it. Svelte
 * brings a leaving block back when its key returns before the exit ends, with its `inert`: what this shut comes back
 * too. They stayed off: «Cancelar» and «Crear códigos nuevos» again within 70 ms left the code field disabled (pass 23). */
function shut(node: Element) {
	if (!(node instanceof HTMLElement)) return;
	node.inert = true;
	const controls = [
		...node.querySelectorAll<HTMLButtonElement | HTMLInputElement>('button, input')
	].filter((control) => !control.disabled);
	for (const control of controls) control.disabled = true;
	// Its display too: hidden at once under reduced motion, a step brought back stayed hidden, unfocusable (pass 71).
	const display = node.style.display;
	const back = new MutationObserver(() => {
		if (node.inert) return;
		back.disconnect();
		node.style.display = display;
		for (const control of controls) control.disabled = false;
	});
	back.observe(node, { attributes: true, attributeFilter: ['inert'] });
}

/** Out of a `{#key}` block that swaps one view for another: a quick fade, shut ([`shut`]). */
export function swapOut(node: Element): TransitionConfig {
	shut(node);
	return vanish(node);
}

/** Out of a step (a `{#key}` block), shut ([`shut`]): a quick fade, sliding to the side it leaves by. */
export function stepOut(node: HTMLElement, { direction = 1 } = {}): TransitionConfig {
	shut(node);
	if (still()) return goneBehindPage(node, DURATION.medium);
	return fly(node, {
		x: still() ? 0 : -32 * direction,
		duration: DURATION.short,
		easing: EASE.exit
	});
}

/** Material 3's fade through, for content replaced in place by a new answer (a new period's figures): the old fades
 * out (35 % of 300 ms), `change` swaps it while nothing shows, and the new fades in, growing a little (65 %). The
 * fade out starts from the element's current opacity, so a dimmed «loading» state does not flash back first. With
 * reduced motion it only fades. */
export async function fadeThrough(
	element: HTMLElement | undefined,
	change: () => void | Promise<void>
): Promise<void> {
	if (!element) {
		await change();
		return;
	}
	// From the opacity on screen: a lone end keyframe starts from the style's value, so a dim still on its way (0.97)
	// went to its end (0.6) in one frame before fading (pass 15).
	const out = element.animate([{ opacity: getComputedStyle(element).opacity }, { opacity: 0 }], {
		duration: 105,
		easing: EASE_CSS.exit,
		fill: 'forwards'
	});
	await out.finished.catch(() => {});
	await change();
	await tick();
	const enter = element.animate(
		[
			{ opacity: 0, transform: still() ? 'none' : 'scale(0.98)' },
			{ opacity: 1, transform: 'none' }
		],
		{ duration: 195, easing: EASE_CSS.enter }
	);
	// The held end of the fade out would hide the element again once the fade in is over.
	out.cancel();
	await enter.finished.catch(() => {});
}

/** A short shake that says "not this one"; never alone, always next to the message that says why. */
export function shake(element: HTMLElement | undefined) {
	if (!element || still()) return;
	const offsets = [0, -8, 7, -5, 3, 0];
	element.animate(
		offsets.map((x) => ({ transform: `translateX(${x}px)` })),
		{ duration: 360, easing: EASE_CSS.standard }
	);
}

/** Where an element sits inside `root`, from layout alone (transforms do not count), borders included. */
function layoutPlace(element: HTMLElement, root: HTMLElement) {
	let [left, top] = [root.clientLeft, root.clientTop];
	let current: Element | null = element;
	while (current instanceof HTMLElement && current !== root) {
		left += current.offsetLeft;
		top += current.offsetTop;
		current = current.offsetParent;
	}
	return { left, top };
}

/** A box's padding and borders, top and bottom: what its height holds besides its content (border-box sizing). */
function frame(node: HTMLElement): number {
	const style = getComputedStyle(node);
	const sides = ['paddingTop', 'paddingBottom', 'borderTopWidth', 'borderBottomWidth'] as const;
	return sides.reduce((sum, side) => sum + parseFloat(style[side]), 0);
}

/**
 * Keeps the element's height following its first child's (plus its own padding and borders), so the card grows or shrinks
 * instead of jumping; the height transition is in CSS, under a no-preference media rule. What sits below a change
 * and is marked `data-flip` slides to its new place with a transform that starts in the same frame as the height,
 * with the same length and curve, so its bottom edge tracks the box's and nothing is clipped (the FLIP technique).
 * The element has to be positioned, so the positions inside it are measured from it. While `held` it only grows: the
 * room it would give back stays until it is let go, so the page can first move where it will stand (see `showWhole`).
 */
export function followContent(node: HTMLElement, held = false) {
	const content = node.firstElementChild;
	if (!(content instanceof HTMLElement)) return;
	let hold = held;
	let last = new Map<HTMLElement, ReturnType<typeof layoutPlace>>();
	// The end of the glide, or a timer if none ran. A cancel is no end: a newer glide replaced it and is still running.
	let settle: ReturnType<typeof setTimeout> | undefined;
	const rest = () => delete node.dataset.moving;
	const end = (event: TransitionEvent) => {
		if (event.target === node && event.propertyName === 'height') rest();
	};
	const fit = () => {
		const height = content.offsetHeight + frame(node);
		if (hold && height < node.offsetHeight) return;
		const first = node.style.height === '';
		const glide = !still() && !first;
		const now: typeof last = new Map();
		for (const element of node.querySelectorAll<HTMLElement>('[data-flip]')) {
			const place = layoutPlace(element, node);
			now.set(element, place);
			const before = last.get(element);
			if (!glide || !before) continue;
			const [x, y] = [before.left - place.left, before.top - place.top];
			if (!x && !y) continue;
			// Hold the old place until the animation takes over, so no frame shows the new one first.
			const from = `translate(${x}px, ${y}px)`;
			element.style.transform = from;
			const animation = element.animate([{ transform: from }, { transform: 'none' }], {
				duration: DURATION.medium,
				easing: EASE_CSS.standard
			});
			animation.ready.then(() => (element.style.transform = ''));
		}
		last = now;
		// While the height glides the box is marked, so a card can clip its content then and only then: at rest a
		// focus ring that reaches past the box shows whole (P3 round 1). Set here, before the frame is painted.
		if (glide && node.style.height !== `${height}px`) {
			node.dataset.moving = '';
			clearTimeout(settle);
			settle = setTimeout(rest, DURATION.medium + 100);
		}
		if (still()) fadeWhatFollows(node, height, DURATION.medium);
		node.style.height = `${height}px`;
		// From `auto` a transition goes on tracking the content: a new card went 130 to 523 px in one frame (pass 42).
		if (first) node.getAnimations().forEach((a) => a instanceof CSSTransition && a.cancel());
	};
	// A resize is seen after the frame's layout, a step after scripts that read it there; a change to the content (a
	// row in or out of the flow) is seen at once, so the rows hold their places from the very frame it happens (P3
	// round 2: a frame read before the observer showed them at the new place).
	const observer = new ResizeObserver(fit);
	const changes = new MutationObserver(fit);
	node.addEventListener('transitionend', end);
	observer.observe(content);
	changes.observe(content, { childList: true, subtree: true, attributeFilter: ['class'] });
	return {
		update(next: boolean) {
			hold = next;
			if (!hold) fit();
		},
		destroy: () => {
			observer.disconnect();
			changes.disconnect();
			clearTimeout(settle);
			node.removeEventListener('transitionend', end);
		}
	};
}

/** The room a box that follows its content would give back once let go (`followContent`, held). */
export function spare(node: HTMLElement): number {
	const content = node.firstElementChild;
	if (!(content instanceof HTMLElement)) return 0;
	return Math.max(0, node.offsetHeight - content.offsetHeight - frame(node));
}
