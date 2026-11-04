// The browser's view transition, which cross-fades the whole page, with a way out. When the window changes size just
// as the update ends (a phone turned, a window resized), Chromium can leave the transition waiting for its first
// frame: it paints nothing, fires no `resize` and ends no animation until its own 4 s timeout, so the screen stands
// still and every exit on the page waits with it. A transition is ready within about 20 ms of its update; one that is
// not by the time it should be is skipped, and the page shows at once, without the fade (D19).
import { DURATION } from '#lib/motion.ts';

export function crossFade(update: () => void | Promise<void>) {
	const transition = document.startViewTransition(update);
	transition.updateCallbackDone.then(
		() => {
			const stuck = setTimeout(() => transition.skipTransition(), DURATION.medium);
			const started = () => clearTimeout(stuck);
			transition.ready.then(started, started);
		},
		// An update that fails, like a navigation that was replaced, ends the transition by itself.
		() => {}
	);
}
