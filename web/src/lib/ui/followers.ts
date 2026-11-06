// What follows a box whose height changes under reduced motion. Nothing may slide there, so the rows below would stand
// at their new place from one frame to the next; they fade in there instead.
const widths = new WeakMap<HTMLElement, number>();

// Landings on their way, and the dims that wait for them.
let landings = 0;
const waiting = new Set<Animation>();

/** The whole page comes back through a fade: what stands in it is about to be elsewhere, with no way to slide there. From dim,
 * not blank: a landing just after an answer blanked the page under the focus (pass 45). It stays dim for two frames: what
 * moves a frame after the fade began moved at 0.34 (pass 46). */
export function fadePage(duration: number, page = document.querySelector('main')) {
	const dim = page?.animate(
		{ opacity: [0.3, 0.3, 1], offset: [0, 0.14, 1] },
		{ id: 'page', duration }
	);
	if (dim && landings) hold(dim);
}

function hold(dim: Animation) {
	dim.pause();
	waiting.add(dim);
}

/** Whatever moved first, a press's landing moves the page again, up to 200 ms later: the page stays dim for it, or it dimmed
 * twice for one press (pass 55). With the last landing over it comes back, once. */
export function landingBegins() {
	landings += 1;
	for (const dim of document.getAnimations()) if (dim.id === 'page') hold(dim);
}

export function landingEnds() {
	landings -= 1;
	if (landings) return;
	for (const dim of waiting) dim.play();
	waiting.clear();
}

/** A fade over what has already taken its place: nothing moves in it. */
export const fadesInPlace = ({ id }: Animation) => id === 'page' || id === 'follow';

/** Whether a box giving back `room` takes the page, or the sheet that scrolls it, to a new end from where it stands. */
export function endsPage(room: number, page: Element = document.documentElement) {
	return room > 0 && page.scrollTop > Math.max(0, page.scrollHeight - page.clientHeight - room);
}

/** Out where its box takes a new height at once and, centred, it would stand elsewhere: 97 px higher, at setup (pass 44). */
export function goneBehindPage(node: HTMLElement, duration: number) {
	node.style.display = 'none';
	fadePage(duration);
	return { duration: 1 };
}

/** Whether an animation ends fainter than it began. */
function leaves({ effect }: Animation) {
	const frames = effect instanceof KeyframeEffect ? effect.getKeyframes() : [];
	return Number(frames.at(-1)?.opacity ?? 1) < Number(frames[0]?.opacity ?? 1);
}

/** Fades in what follows `box` inside the page, which has just taken its new place. `again`: it moved again, so a fade on its
 * way starts over, or the second place would land in plain sight. */
export function fadeAfter(box: Element, duration: number, again = false) {
	const page = box.closest('main');
	if (!page) return;
	for (let at: Element | null = box; at && at !== page; at = at.parentElement) {
		for (let next = at.nextElementSibling; next; next = next.nextElementSibling) {
			const running = next.getAnimations();
			const fading = running.filter((animation) => animation.id === 'follow');
			const own = running.filter((animation) => animation.id !== 'follow');
			// On its way out it goes now, with no way back: fading in, a reading leaving came back for a moment (pass 56). On its
			// way in it has its own fade.
			if (own.some(leaves)) next.animate({ opacity: [0, 0] }, { id: 'follow', duration });
			if (own.length || (fading.length && !again)) continue;
			for (const animation of fading) animation.cancel();
			next.animate({ opacity: [0, 1] }, { id: 'follow', duration });
		}
	}
}

/** A list item's way to a new place where nothing may slide: it fades in where it stands, after fading out for `out` ms where
 * it stood, `up` px below. Landing at once, rows jumped 140 to 313 px at full opacity on each choice (pass 40). */
export function fadeToPlace(up: number, out: number, duration: number) {
	if (Math.abs(up) < 4) return { duration: 0 };
	const turn = out / (out + duration);
	return {
		duration: out + duration,
		css: (t: number) =>
			t < turn
				? `opacity: ${1 - t / turn}; transform: translateY(${up}px)`
				: `opacity: ${(t - turn) / (1 - turn)}; transform: none`
	};
}

/** `fadeAfter` for a box about to take `height`. A resize is left alone: the text rewraps at each of its steps, and a
 * fade for each would blink. So is a change of a few pixels, which nobody sees. Where it ends the page, what stands above
 * it moves too, 294 px at full opacity as an answer left (pass 51), and the whole page fades. */
export function fadeWhatFollows(box: HTMLElement, height: number, duration: number) {
	const width = box.offsetWidth;
	const resized = widths.get(box) !== width;
	widths.set(box, width);
	if (resized || !box.style.height || Math.abs(box.offsetHeight - height) < 4) return;
	fadeAfter(box, duration);
	if (endsPage(box.offsetHeight - height)) fadePage(duration);
}
