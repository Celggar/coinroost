/**
 * Keeps a box on whole device pixels. Centred by its layout, the sign-in card fell between two, and what inside it
 * paints on a layer of its own (a pressed button, a hover's tint, a list popping in) was rounded 1 px away from what
 * painted around it: «Copiados» stood 1 px above «Descargar» (pass 6). The box moves by less than a pixel, and again
 * whenever it or the window changes size. The box must be positioned (relative).
 */
export function wholePixels(node: HTMLElement, watched: HTMLElement = node) {
	let x = 0;
	let y = 0;
	// The layout places boxes in steps of 1/64 px. A nudge between two steps was rounded, the next snap read that
	// rounding as the layout's own place, and every third snap left the card 1/32 px off (pass 13 gates): nudges are
	// whole steps, so the place without them is exact.
	const step = 1 / 64;
	const nudge = (at: number, ratio: number) =>
		Math.round((Math.round(at * ratio) / ratio - at) / step) * step;
	const snap = () => {
		const ratio = devicePixelRatio || 1;
		const box = node.getBoundingClientRect();
		// Where the layout puts it, without the nudge already given, in the page (the scroll moves whole pixels).
		const left = box.left - x + scrollX;
		const top = box.top - y + scrollY;
		x = nudge(left, ratio);
		y = nudge(top, ratio);
		// A relative offset, not a transform: under a transform the browser stops snapping borders to pixels.
		node.style.left = x ? `${x}px` : '';
		node.style.top = y ? `${y}px` : '';
	};
	const watch = new ResizeObserver(snap);
	watch.observe(watched);
	watch.observe(document.documentElement);
	addEventListener('resize', snap);
	snap();
	return {
		destroy() {
			watch.disconnect();
			removeEventListener('resize', snap);
		}
	};
}
