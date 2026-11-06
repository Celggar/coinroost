import { tick } from 'svelte';
import { prefersReducedMotion } from 'svelte/motion';
import { fade, slide, type TransitionConfig } from 'svelte/transition';
import {
	DURATION,
	EASE,
	EASE_CSS,
	FADED,
	fadeThrough,
	glide,
	swapIn,
	vanish
} from '#lib/motion.ts';
import { endsPage, fadeAfter, fadePage } from '#lib/ui/followers.ts';

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
	// Measured with the room still held: let go first, the read laid the page out short and the browser took the scroll to
	// that end, 149 to 238 px in a frame, before the animation held it again (pass 50).
	const staying = [...box.children].filter((e) => e instanceof HTMLElement && !e.inert);
	const after = Math.max(0, ...staying.map((e) => (e as HTMLElement).offsetHeight));
	if (after >= before) {
		box.style.minHeight = '';
		return letGo();
	}
	if (prefersReducedMotion.current) {
		// Whether the page ends is asked as the room goes: another box may have given back its own since, and neither alone
		// reached the end, 232 px at full opacity (pass 52).
		setTimeout(() => {
			const ends = endsPage(before - after);
			box.style.minHeight = '';
			letGo();
			fadeAfter(box, DURATION.medium);
			if (ends) fadePage(DURATION.medium);
		}, FADED);
		return;
	}
	const room = box.animate([{ minHeight: `${before}px` }, { minHeight: `${after}px` }], {
		delay: FADED,
		duration: DURATION.medium,
		easing: EASE_CSS.standard,
		fill: 'backwards'
	});
	box.style.minHeight = '';
	room.oncancel = letGo;
	room.onfinish = letGo;
}

/**
 * From the height `before` to the one the box has now, in a glide, so what follows moves with it: figures swapped while
 * faded out took a ranking's row with them, and the card under them went 67 px in one frame (pass 37). With reduced motion
 * it goes at once. The box must not stretch what it holds (`align-content: start`).
 */
export function glideRoom(box: HTMLElement, before: number, after = box.offsetHeight) {
	if (prefersReducedMotion.current || Math.abs(after - before) < 1) return;
	const room = (height: number) => ({ height: `${height}px`, overflow: 'clip' });
	box.animate([room(before), room(after)], {
		duration: DURATION.medium,
		easing: EASE_CSS.standard
	});
}

/** A list's first row takes the list's whole room, from the `before` of what stood there: it came in a frame, 198 px with
 * the form under it at full opacity (pass 48). */
export function firstRoom(box: HTMLElement | null | undefined, before: number) {
	if (!box) return;
	glideRoom(box, before);
	if (prefersReducedMotion.current) fadePage(DURATION.medium);
}

/** With no glide a box giving back `room` takes the page to its new end at once, all of it: it comes back through a fade. */
function fadeIfPageMoves(room: number) {
	if (endsPage(room)) fadePage(DURATION.medium);
}

/**
 * A list that changes as a whole, behind a fade, its room gliding: a filter's rows changed in one frame, the list 372 to
 * 853 px (pass 40), and rows sorted by a rule jumped 170 px at full opacity (pass 41). Meanwhile its rows change places with
 * no motion of their own (`leave`, `move`). `box` follows its first child's height.
 */
export function behindFade(box: () => HTMLElement | undefined) {
	let going = false;
	return {
		going: () => going,
		async run(change: () => unknown) {
			const list = box();
			going = true;
			await fadeThrough(list, async () => {
				const room = list?.offsetHeight ?? 0;
				// Held until the glide has it: reading the new height with the box already short took the page to its new
				// end at once, 120 px in one frame, before the glide began (pass 41).
				if (list) list.style.minHeight = `${room}px`;
				await change();
				await tick();
				const content = list?.firstElementChild;
				if (!list || !(content instanceof HTMLElement)) return;
				const after = content.offsetHeight;
				glideRoom(list, room, after);
				if (prefersReducedMotion.current) fadeIfPageMoves(room - after);
				list.style.minHeight = '';
			});
			going = false;
		},
		// Out of the page at once, gone a moment later: in a group of leaving rows Svelte 5.57 counts one that ends at once as still
		// waiting if another really waits (the chosen row's answer), and none of them ever left.
		leave(node: HTMLElement) {
			if (!going) return vanishRow(node);
			node.style.display = 'none';
			return { duration: 1 };
		},
		/** `change` takes the whole block away, behind a fade of the page. The last rows sorted took the page back 1307 px in a
		 * frame (pass 43), and gliding there, the focus stood above the window until it landed (pass 44). */
		async whole(change: () => unknown) {
			going = true;
			await fadeThrough(box()?.closest('main') ?? undefined, async () => {
				await change();
			});
			going = false;
		},
		leaveAll(node: HTMLElement) {
			if (!going) return vanish(node);
			node.style.display = 'none';
			return { duration: 1 };
		},
		arrive: (node: Element) => (going ? { duration: 0 } : swapIn(node)),
		move: (node: Element, places: { from: DOMRect; to: DOMRect }) =>
			going ? { duration: 0 } : glide(node, places)
	};
}

/**
 * A form opening or closing in place among a page's blocks, the gap above it with it: the gap came at once, and what
 * follows stood 16 px lower before anything had begun to open. With reduced motion it only fades (`follow` for what follows).
 */
export function openRoom(node: Element, { delay = 0 } = {}): TransitionConfig {
	const page = node.parentElement;
	const gap = page ? parseFloat(getComputedStyle(page).rowGap) || 0 : 0;
	if (prefersReducedMotion.current) {
		if (!delay) return fade(node, { duration: DURATION.short });
		// Waiting, it takes no room: the room comes in one go, when what it waits for has gone.
		const height = node.getBoundingClientRect().height;
		const room = (t: number) =>
			t > 0 ? `height: ${height}px; margin-top: 0px;` : `height: 0px; margin-top: ${-gap}px;`;
		return {
			delay,
			duration: DURATION.short,
			css: (t) => `overflow: hidden; opacity: ${t}; ${room(t)}`
		};
	}
	const room = slide(node, { delay, duration: DURATION.medium, easing: EASE.standard });
	return { ...room, css: (t, u) => `${room.css?.(t, u)}; margin-top: ${-u * gap}px` };
}

/** For the document's clicks. A fold opens as the click ends and its `toggle` comes a frame later, in plain sight. */
export function foldOpens(event: MouseEvent) {
	const summary = (event.target as Element).closest('main details:not([open]) > summary');
	if (summary && prefersReducedMotion.current) fadeAfter(summary, DURATION.medium, true);
}

/** The fold a marker transition (`--fold`) says is closing, where nothing may slide. */
function closing({ propertyName, target: fold }: TransitionEvent) {
	if (propertyName !== '--fold' || !prefersReducedMotion.current) return null;
	return fold instanceof HTMLDetailsElement && !fold.open ? fold : null;
}

// Closing folds whose room holds up the end of what scrolls them.
const holdingEnd = new WeakSet<HTMLDetailsElement>();

/** For the document's `transitionrun`: the room is still there to measure, and gone by the time `foldClosed` could ask. */
export function foldCloses(event: TransitionEvent) {
	const fold = closing(event);
	if (!fold) return;
	const room = fold.offsetHeight - (fold.querySelector('summary')?.offsetHeight ?? 0);
	if (endsPage(room, fold.closest('dialog') ?? undefined)) holdingEnd.add(fold);
	else holdingEnd.delete(fold);
}

/** For the document's `transitionend`. A fold keeps its room while its body fades and gives it back as that ends: what
 * follows fades in then, before the frame shows; faded at the `toggle`, it stood 164 px higher at full opacity. At the end
 * of what scrolls it all of that moves, 389 px on a phone (pass 48), and comes back through a fade: a sheet stands over the
 * page, so its own form does. */
export function foldClosed(event: TransitionEvent) {
	const fold = closing(event);
	if (!fold) return;
	if (holdingEnd.delete(fold)) fadePage(DURATION.medium, fold.closest('dialog form, main'));
	else fadeAfter(fold, DURATION.medium, true);
}

/** For a block's `onintrostart` and `onoutroend`: under reduced motion its room comes and goes at once, so what follows
 * it fades in at its new place. At the page's end the page goes with it: 168 px in a frame as an offer closed (pass 45). */
export function follow(event: Event, held = false) {
	if (!prefersReducedMotion.current) return;
	const box = event.currentTarget as HTMLElement;
	if (box instanceof HTMLDetailsElement && !box.open) return;
	fadeAfter(box, DURATION.medium, true);
	if (event.type === 'outroend' && !held) fadeIfPageMoves(box.offsetHeight);
}

/**
 * Out of a list that lends its rows its columns (subgrid): out of the flow a row has no columns to borrow, and
 * its line broke in three at full opacity on every choice (pass 38). It keeps them, in numbers, as it fades.
 */
export function vanishRow(node: HTMLElement): TransitionConfig {
	const list = node.parentElement;
	if (list) node.style.gridTemplateColumns = getComputedStyle(list).gridTemplateColumns;
	return vanish(node);
}
