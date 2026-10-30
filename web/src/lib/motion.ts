// Transitions for things that appear and disappear. They follow the Material 3 motion tokens (in app.css): entering
// decelerates, leaving accelerates and is faster. With the reduced-motion setting on, things still fade but nothing
// moves or grows (MDN: replace the motion, do not only remove it). Svelte runs these with the Web Animations API,
// which CSS media rules do not reach, so each one reads the setting itself.
import { backOut, cubicIn, quintOut } from 'svelte/easing';
import { prefersReducedMotion } from 'svelte/motion';
import { fade, fly, scale, type TransitionConfig } from 'svelte/transition';

/** Milliseconds, as in app.css. */
export const DURATION = { short: 150, exit: 200, medium: 300, long: 500 } as const;

const still = () => prefersReducedMotion.current;

/** In: rises a little into place. */
export function rise(node: Element, { delay = 0, y = 12 } = {}): TransitionConfig {
	return fly(node, { delay, y: still() ? 0 : y, duration: DURATION.medium, easing: quintOut });
}

/** Out: lifts away, faster than it came. */
export function lift(node: Element, { y = -8 } = {}): TransitionConfig {
	return fly(node, { y: still() ? 0 : y, duration: DURATION.exit, easing: cubicIn });
}

/** In: grows from a little smaller, with a slight overshoot. */
export function pop(node: Element, { delay = 0, start = 0.6 } = {}): TransitionConfig {
	return scale(node, {
		delay,
		start: still() ? 1 : start,
		duration: DURATION.medium,
		easing: backOut
	});
}

/** Out: a quick fade. */
export function vanish(node: Element): TransitionConfig {
	return fade(node, { duration: DURATION.short, easing: cubicIn });
}

/** In and out of a step: slides in from the side it comes from (`direction` 1 forward, -1 back). */
export function stepIn(node: Element, { direction = 1 } = {}): TransitionConfig {
	return fly(node, {
		x: still() ? 0 : 32 * direction,
		duration: DURATION.medium,
		easing: quintOut
	});
}

export function stepOut(node: Element, { direction = 1 } = {}): TransitionConfig {
	return fly(node, { x: still() ? 0 : -32 * direction, duration: DURATION.exit, easing: cubicIn });
}

/** A short shake that says "not this one"; never alone, always next to the message that says why. */
export function shake(element: HTMLElement | undefined) {
	if (!element || still()) return;
	const offsets = [0, -8, 7, -5, 3, 0];
	element.animate(
		offsets.map((x) => ({ transform: `translateX(${x}px)` })),
		{ duration: 360, easing: 'ease-in-out' }
	);
}

/**
 * Keeps the element's height following its first child's (plus its own padding), so a box whose content changes
 * grows or shrinks instead of jumping. The transition itself is in CSS, under a no-preference media rule.
 */
export function followHeight(node: HTMLElement) {
	const content = node.firstElementChild;
	if (!(content instanceof HTMLElement)) return;
	const observer = new ResizeObserver(() => {
		const style = getComputedStyle(node);
		const padding = parseFloat(style.paddingTop) + parseFloat(style.paddingBottom);
		node.style.height = `${content.offsetHeight + padding}px`;
	});
	observer.observe(content);
	return { destroy: () => observer.disconnect() };
}
