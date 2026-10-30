// Transitions for things that appear and disappear. They follow the Material 3 motion tokens (in app.css): entering
// decelerates, leaving accelerates and is faster. With the reduced-motion setting on, things still fade but nothing
// moves or grows (MDN: replace the motion, do not only remove it). Svelte runs these with the Web Animations API,
// which CSS media rules do not reach, so each one reads the setting itself.
import { prefersReducedMotion } from 'svelte/motion';
import { fade, fly, scale, type TransitionConfig } from 'svelte/transition';

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

/** In: rises a little into place. A delay that waits for the card to grow is dropped when nothing grows. */
export function rise(node: Element, { delay = 0, y = 12 } = {}): TransitionConfig {
	return fly(node, {
		delay: still() ? 0 : delay,
		y: still() ? 0 : y,
		duration: DURATION.medium,
		easing: EASE.enter
	});
}

/** Out: lifts away, faster than it came. */
export function lift(node: Element, { y = -8 } = {}): TransitionConfig {
	return fly(node, { y: still() ? 0 : y, duration: DURATION.exit, easing: EASE.exit });
}

/** In: grows from a little smaller. */
export function pop(node: Element, { delay = 0, start = 0.6 } = {}): TransitionConfig {
	return scale(node, {
		delay,
		start: still() ? 1 : start,
		duration: DURATION.medium,
		easing: EASE.enter
	});
}

/** Out: a quick fade. */
export function vanish(node: Element): TransitionConfig {
	return fade(node, { duration: DURATION.short, easing: EASE.exit });
}

/** In and out of a step: slides in from the side it comes from (`direction` 1 forward, -1 back). */
export function stepIn(node: Element, { direction = 1 } = {}): TransitionConfig {
	return fly(node, {
		x: still() ? 0 : 32 * direction,
		duration: DURATION.medium,
		easing: EASE.enter
	});
}

export function stepOut(node: Element, { direction = 1 } = {}): TransitionConfig {
	return fly(node, {
		x: still() ? 0 : -32 * direction,
		duration: DURATION.exit,
		easing: EASE.exit
	});
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

/** Where an element sits inside `root`, from layout alone (transforms do not count). */
function layoutTop(element: HTMLElement, root: HTMLElement) {
	let top = 0;
	let current: Element | null = element;
	while (current instanceof HTMLElement && current !== root) {
		top += current.offsetTop;
		current = current.offsetParent;
	}
	return top;
}

/**
 * Keeps the element's height following its first child's (plus its own padding), so the card grows or shrinks
 * instead of jumping; the height transition is in CSS, under a no-preference media rule. What sits below a change
 * and is marked `data-flip` slides to its new place with a transform that starts in the same frame as the height,
 * with the same length and curve, so its bottom edge tracks the box's and nothing is clipped (the FLIP technique).
 * The element has to be positioned, so the positions inside it are measured from it.
 */
export function followContent(node: HTMLElement) {
	const content = node.firstElementChild;
	if (!(content instanceof HTMLElement)) return;
	let last = new Map<HTMLElement, number>();
	const observer = new ResizeObserver(() => {
		const style = getComputedStyle(node);
		const padding = parseFloat(style.paddingTop) + parseFloat(style.paddingBottom);
		const glide = !still() && node.style.height !== '';
		const now = new Map<HTMLElement, number>();
		for (const element of node.querySelectorAll<HTMLElement>('[data-flip]')) {
			const top = layoutTop(element, node);
			now.set(element, top);
			const before = last.get(element);
			if (!glide || before === undefined || before === top) continue;
			// Hold the old place until the animation takes over, so no frame shows the new one first.
			element.style.transform = `translateY(${before - top}px)`;
			const animation = element.animate(
				[{ transform: `translateY(${before - top}px)` }, { transform: 'none' }],
				{ duration: DURATION.medium, easing: EASE_CSS.standard }
			);
			animation.ready.then(() => (element.style.transform = ''));
		}
		last = now;
		node.style.height = `${content.offsetHeight + padding}px`;
	});
	observer.observe(content);
	return { destroy: () => observer.disconnect() };
}
