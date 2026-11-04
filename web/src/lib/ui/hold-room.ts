import { tick } from 'svelte';
import { prefersReducedMotion } from 'svelte/motion';
import { DURATION, EASE_CSS, FADED } from '#lib/motion.ts';

/**
 * Keeps a list's room while `change` takes a row out of it, and gives the room back with the rows' own wait and glide
 * (`glide`), so what follows the list moves with them. A row that leaves goes out of the flow at once, and «Regla nueva»
 * went up 238 px in one frame, under the rule still fading (pass 25). What is leaving has no say in the height to come:
 * the whole list, going as its last row does, makes way for what replaces it. With reduced motion the room waits as
 * long, then goes at once, as the rows do.
 */
export async function holdRoom(box: HTMLElement | null, change: () => unknown) {
	if (!box) return change();
	const before = box.offsetHeight;
	box.style.minHeight = `${before}px`;
	await change();
	await tick();
	box.style.minHeight = '';
	// Measured at their own height: in one grid cell, what stays stretches to what is leaving.
	box.style.alignItems = 'start';
	const staying = [...box.children].filter((e) => e instanceof HTMLElement && !e.inert);
	const after = Math.max(0, ...staying.map((e) => (e as HTMLElement).offsetHeight));
	box.style.alignItems = '';
	if (after >= before) return;
	box.animate([{ minHeight: `${before}px` }, { minHeight: `${after}px` }], {
		delay: FADED,
		duration: prefersReducedMotion.current ? 0 : DURATION.medium,
		easing: EASE_CSS.standard,
		fill: 'backwards'
	});
}
