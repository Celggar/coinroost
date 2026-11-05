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
	// What the box holds keeps its own height: stretched into the room, a list's rows grew over the one leaving (pass 26).
	box.style.alignItems = 'start';
	const letGo = () => box.style.removeProperty('align-items');
	await change();
	await tick();
	box.style.minHeight = '';
	const staying = [...box.children].filter((e) => e instanceof HTMLElement && !e.inert);
	const after = Math.max(0, ...staying.map((e) => (e as HTMLElement).offsetHeight));
	if (after >= before) return letGo();
	const room = box.animate([{ minHeight: `${before}px` }, { minHeight: `${after}px` }], {
		delay: FADED,
		duration: prefersReducedMotion.current ? 0 : DURATION.medium,
		easing: EASE_CSS.standard,
		fill: 'backwards'
	});
	room.onfinish = room.oncancel = letGo;
}

/**
 * From the height `before` to the one the box has now, in a glide, so what follows moves with it: figures swapped while
 * faded out took a ranking's row with them, and the card under them went 67 px in one frame (pass 37). With reduced motion
 * it goes at once. The box must not stretch what it holds (`align-content: start`).
 */
export function glideRoom(box: HTMLElement, before: number) {
	const after = box.offsetHeight;
	if (prefersReducedMotion.current || Math.abs(after - before) < 1) return;
	const room = (height: number) => ({ height: `${height}px`, overflow: 'clip' });
	box.animate([room(before), room(after)], {
		duration: DURATION.medium,
		easing: EASE_CSS.standard
	});
}
