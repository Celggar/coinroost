import { tick } from 'svelte';
import { prefersReducedMotion } from 'svelte/motion';
import type { TransitionConfig } from 'svelte/transition';
import { DURATION, EASE_CSS, FADED, fadeThrough, glide, swapIn, vanish } from '#lib/motion.ts';
import { fadeAfter, fadePage } from '#lib/ui/followers.ts';

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
export function glideRoom(box: HTMLElement, before: number, after = box.offsetHeight) {
	if (prefersReducedMotion.current || Math.abs(after - before) < 1) return;
	const room = (height: number) => ({ height: `${height}px`, overflow: 'clip' });
	box.animate([room(before), room(after)], {
		duration: DURATION.medium,
		easing: EASE_CSS.standard
	});
}

/** With no glide a box giving back `room` takes the page to its new end at once, all of it: it comes back through a fade. */
function fadeIfPageMoves(box: HTMLElement, room: number) {
	const end = document.documentElement.scrollHeight - innerHeight - room;
	if (room > 0 && scrollY > end) fadePage(DURATION.medium);
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
				if (prefersReducedMotion.current) fadeIfPageMoves(list, room - after);
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

/** For a block's `onintrostart` and `onoutroend`: under reduced motion its room comes and goes at once, so what follows
 * it fades in at its new place. */
export function follow(event: Event) {
	if (prefersReducedMotion.current) fadeAfter(event.currentTarget as Element, DURATION.medium);
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
