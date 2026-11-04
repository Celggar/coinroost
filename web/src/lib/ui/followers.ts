// What follows a box whose height changes under reduced motion. Nothing may slide there, so the rows below would stand
// at their new place from one frame to the next; they fade in there instead.
const widths = new WeakMap<HTMLElement, number>();

/** Fades in what follows `box` inside the page, as the box is about to take `height`. A resize is left alone: the text
 * rewraps at each of its steps, and a fade for each would blink. So is a change of a few pixels, which nobody sees. */
export function fadeWhatFollows(box: HTMLElement, height: number, duration: number) {
	const width = box.offsetWidth;
	const resized = widths.get(box) !== width;
	widths.set(box, width);
	const page = box.closest('main');
	if (!page || resized || !box.style.height || Math.abs(box.offsetHeight - height) < 4) return;
	for (let at: Element | null = box; at && at !== page; at = at.parentElement) {
		for (let next = at.nextElementSibling; next; next = next.nextElementSibling) {
			if (next.getAnimations().some((animation) => animation.id === 'follow')) continue;
			next.animate({ opacity: [0, 1] }, { id: 'follow', duration });
		}
	}
}
