// What the queue does frame by frame during an action: where each row stands in the page, how much of it shows (its own
// opacity and its list's), the page's scroll and height.
import { expect, type Page } from '@playwright/test';

type Move = { step: number; shown: number };
type Frames = {
	moves: Move[];
	gone: number[];
	crossed: number;
	scrolls: number[];
	pages: number[];
	heights: number[];
	unseen: number;
	frames: number;
};

const FRAMES = 60;

/** Starts recording: every step of a row beyond 2 px, every fall of a row's opacity in one frame, a row taken away included. */
export async function watchRows(page: Page) {
	await page.evaluate((frames) => {
		const w = window as unknown as Frames;
		const places = new Map<Element, { top: number; shown: number }>();
		const opacity = (e: Element | null) => {
			if (!e) return 1;
			return e.checkVisibility() ? Number(getComputedStyle(e).opacity) : 0;
		};
		w.moves = [];
		w.gone = [];
		w.crossed = 0;
		w.scrolls = [];
		w.pages = [];
		w.heights = [];
		w.unseen = 0;
		w.frames = 0;
		const step = () => {
			const rows = new Set(document.querySelectorAll('.queue > li'));
			for (const [row, was] of places) {
				if (!rows.has(row) && was.shown > 0.4) w.gone.push(was.shown);
				if (!rows.has(row)) places.delete(row);
			}
			for (const row of rows) {
				const top = row.getBoundingClientRect().top + scrollY;
				const shown = opacity(row) * opacity(row.closest('.follows'));
				const was = places.get(row);
				if (was && Math.abs(top - was.top) > 2) w.moves.push({ step: top - was.top, shown });
				if (was && was.shown - shown > 0.7) w.gone.push(was.shown - shown);
				places.set(row, { top, shown });
			}
			// The offer's form and the answer that takes its place, never both at once.
			const answer = document.querySelector('.more :is(#changes-some, #changes-none)');
			const block = answer?.closest('.in-place > *') ?? null;
			const form = document.querySelector('.offer');
			if (opacity(block) > 0.02 && block && form && opacity(form) > 0.02) w.crossed += 1;
			// The focus on the answer while the answer does not show: nobody sees where it went.
			let shows = 1;
			const focused = document.activeElement;
			if (focused?.matches('#changes-some, #changes-none'))
				for (let e: Element | null = focused; e; e = e.parentElement)
					shows *= Number(getComputedStyle(e).opacity);
			if (shows < 0.1) w.unseen += 1;
			w.scrolls.push(scrollY);
			w.pages.push(opacity(document.querySelector('main')));
			w.heights.push(document.documentElement.scrollHeight);
			if (++w.frames < frames) requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	}, FRAMES);
}

async function recorded(page: Page) {
	await page.waitForFunction((frames) => (window as unknown as Frames).frames >= frames, FRAMES);
	return page.evaluate(() => {
		const { moves, gone, crossed, scrolls, pages, heights, unseen } = window as unknown as Frames;
		return { moves, gone, crossed, scrolls, pages, heights, unseen };
	});
}

/** Rows moved, and none in plain sight: where nothing may slide. */
export async function expectFadedMoves(page: Page, what: string) {
	const { moves } = await recorded(page);
	expect(moves.length, what).toBeGreaterThan(0);
	expect(
		moves.filter(({ shown }) => shown > 0.3),
		what
	).toEqual([]);
}

/** A change that may glide but never jump: no row steps far in sight, none vanishes in a frame, the page's end glides. */
export async function expectNoJump(page: Page, what: string, limit: number) {
	const { moves, gone, crossed, heights, unseen } = await recorded(page);
	expect(
		moves.filter(({ step, shown }) => shown > 0.3 && Math.abs(step) >= limit),
		what
	).toEqual([]);
	expect(gone, what).toEqual([]);
	expect(crossed, what).toBe(0);
	// A frame or two as its fade starts: about 230 ms with no ring, at first (pass 42).
	expect(unseen, what).toBeLessThan(4);
	// Where rows may glide, the page's end glides too, a long way faster, never in one go: 392 px of 392 in a frame (pass 41).
	// Where none may, it lands behind the fade.
	const whole = Math.abs(heights[0] - heights.at(-1)!);
	const steps = heights.slice(1).map((height, i) => Math.abs(height - heights[i]));
	if (limit > 2) expect(Math.max(...steps), what).toBeLessThan(Math.max(limit, whole / 4));
}

/** The page went to its new end with the list's room: gliding, or, where nothing may slide, at once behind a fade. */
export async function expectPageFollows(page: Page, what: string, glides: boolean) {
	const { scrolls, pages } = await recorded(page);
	const steps = scrolls.slice(1).map((y, i) => Math.abs(y - scrolls[i]));
	const whole = Math.abs(scrolls[0] - scrolls.at(-1)!);
	const biggest = Math.max(...steps);
	const trail = [...new Set(scrolls)].join(' ');
	expect(whole, `${what}: ${trail}`).toBeGreaterThan(50);
	if (glides) expect(biggest, `${what}: ${trail}`).toBeLessThan(whole * 0.6);
	else {
		const at = steps.indexOf(biggest);
		expect(Math.min(pages[at], pages[at + 1]), `${what}: ${trail}`).toBeLessThan(0.4);
	}
}

/** Starts recording a new file's card, whatever it holds. */
export async function watchCard(page: Page) {
	await page.evaluate((frames) => {
		const w = window as unknown as { cards: number[] };
		w.cards = [];
		const step = () => {
			const card = document.querySelector<HTMLElement>('article.card');
			if (card) w.cards.push(card.offsetHeight);
			if (w.cards.length < frames) requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	}, FRAMES);
}

/** A new card grows to what it reads gliding: it went 130 to 523 px in one frame as it came in (pass 42). */
export async function expectCardGlides(page: Page) {
	await page.waitForFunction(
		(frames) => (window as unknown as { cards: number[] }).cards.length >= frames,
		FRAMES
	);
	const cards = await page.evaluate(() => (window as unknown as { cards: number[] }).cards);
	const whole = Math.max(...cards) - Math.min(...cards);
	const biggest = Math.max(...cards.slice(1).map((height, i) => Math.abs(height - cards[i])));
	expect(whole, cards.join(' ')).toBeGreaterThan(100);
	expect(biggest, cards.join(' ')).toBeLessThan(whole * 0.6);
}
