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
	away: number;
	frames: number;
};

const FRAMES = 60;

/** The recording has seen a frame. A press sent before that had the page dim from the first frame, and the dim went uncounted. */
const started = (page: Page) =>
	page.waitForFunction(() => (window as unknown as { frames: number }).frames > 0);

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
		w.away = 0;
		w.frames = 0;
		const step = () => {
			const rows = new Set(document.querySelectorAll('.queue > li'));
			for (const [row, was] of places) {
				if (!rows.has(row) && was.shown > 0.4) w.gone.push(was.shown);
				if (!rows.has(row)) places.delete(row);
			}
			for (const row of rows) {
				const top = row.getBoundingClientRect().top + scrollY;
				// The page's fade too: what moves behind it does not show, whatever the list's own opacity says.
				const shown =
					opacity(row) * opacity(row.closest('.follows')) * opacity(row.closest('main'));
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
			const box = focused?.getBoundingClientRect();
			if (focused?.matches('#all-done a') && box && (box.bottom <= 0 || box.top >= innerHeight))
				w.away += 1;
			if (focused?.matches('#changes-some, #changes-none, #all-done a'))
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
	await started(page);
}

async function recorded(page: Page) {
	await page.waitForFunction((frames) => (window as unknown as Frames).frames >= frames, FRAMES);
	return page.evaluate(() => {
		const w = window as unknown as Frames;
		const { moves, gone, crossed, scrolls, pages, heights, unseen, away } = w;
		return { moves, gone, crossed, scrolls, pages, heights, unseen, away };
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
export async function expectPageFollows(page: Page, what: string, glides: boolean, once = false) {
	const { scrolls, pages, unseen, away } = await recorded(page);
	// The focus on an end card reached it as it shows, where it shows: above the window for 82 ms, at first (pass 44).
	expect(away, what).toBe(0);
	expect(unseen, what).toBeLessThan(4);
	const steps = scrolls.slice(1).map((y, i) => Math.abs(y - scrolls[i]));
	const whole = Math.max(...scrolls) - Math.min(...scrolls);
	const trail = [...new Set(scrolls)].join(' ');
	expect(whole, `${what}: ${trail}`).toBeGreaterThan(50);
	if (glides) expect(Math.max(...steps), `${what}: ${trail}`).toBeLessThan(whole * 0.6);
	else {
		const seen = steps.filter((step, i) => step >= 4 && Math.min(pages[i], pages[i + 1]) >= 0.4);
		expect(seen, `${what}: ${trail}`).toEqual([]);
		// One fade for one move: the page dimmed twice, 350 ms apart (pass 47).
		const dims = pages.slice(1).filter((shown, i) => shown < 0.5 && pages[i] >= 0.5);
		if (once) expect(dims.length, `${what}: the page's fades`).toBe(1);
	}
}

/** Starts counting the frames in which a new file's card goes through a height that started from `auto`. */
export async function watchCard(page: Page) {
	await page.evaluate((frames) => {
		const w = window as unknown as { fromAuto: number; left: number };
		w.fromAuto = 0;
		w.left = frames;
		const step = () => {
			const running = document.querySelector('article.card')?.getAnimations() ?? [];
			const from = running.map((a) => (a.effect as KeyframeEffect).getKeyframes()[0]?.height);
			if (from.some((height) => String(height).includes('auto'))) w.fromAuto += 1;
			if (--w.left > 0) requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	}, FRAMES);
}

/** Such a height tracks the content while it runs: on a quick answer a card went 130 to 523 px in a frame, on a slower one
 * less (pass 42), so this counts the cause, which every card has, not a step that depends on the server's time. */
export async function expectCardGlides(page: Page) {
	await page.waitForFunction(() => (window as unknown as { left: number }).left <= 0);
	expect(await page.evaluate(() => (window as unknown as { fromAuto: number }).fromAuto)).toBe(0);
}

/** Starts recording where `selector` stands in the window, and how much of the page shows. */
export async function watchTop(page: Page, selector: string, shade = 'main') {
	await page.evaluate(
		({ selector, shade, frames }) => {
			const w = window as unknown as { tops: [number, number][] };
			const main = document.querySelector(shade)!;
			w.tops = [];
			const step = () => {
				const top = document.querySelector(selector)?.getBoundingClientRect().top ?? 0;
				w.tops.push([top, Number(getComputedStyle(main).opacity)]);
				if (w.tops.length < frames) requestAnimationFrame(step);
			};
			requestAnimationFrame(step);
		},
		{ selector, shade, frames: FRAMES }
	);
}

/** It moved in a glide: no frame took it `limit` px. What left reads 0, and counts no more. */
export async function expectGlidedTop(page: Page, limit: number) {
	await page.waitForFunction(
		(frames) => (window as unknown as { tops: unknown[] }).tops.length >= frames,
		FRAMES
	);
	const tops = await page.evaluate(() => (window as unknown as { tops: [number, number][] }).tops);
	const there = tops.map(([top]) => top).filter((top) => top !== 0);
	const trail = there.map(Math.round).join(' ');
	const steps = there.slice(1).map((top, i) => Math.abs(top - there[i]));
	expect(Math.max(...steps), trail).toBeLessThan(limit);
}

/** Starts recording how far past the bottom of `box`, which clips it, `inner` stands. */
export async function watchCut(page: Page, inner: string, box: string) {
	await page.evaluate(
		({ inner, box, frames }) => {
			const w = window as unknown as { cut: number[]; cutting: number };
			w.cut = [];
			w.cutting = frames;
			const step = () => {
				const bottom = (selector: string) =>
					document.querySelector(selector)?.getBoundingClientRect().bottom ?? 0;
				const past = bottom(inner) - bottom(box);
				if (past > 1) w.cut.push(Math.round(past));
				if (--w.cutting > 0) requestAnimationFrame(step);
			};
			requestAnimationFrame(step);
		},
		{ inner, box, frames: FRAMES }
	);
}

/** It stayed whole: the button pressed went out through its card's edge for 150 ms (pass 63). */
export async function expectNotCut(page: Page, what: string) {
	await page.waitForFunction(() => (window as unknown as { cutting: number }).cutting <= 0);
	const cut = await page.evaluate(() => (window as unknown as { cut: number[] }).cut);
	expect(cut, what).toEqual([]);
}

/** It moved, and never in plain sight: where nothing may slide, only behind the page's fade. `once`: behind a single dim. */
export async function expectFadedTop(page: Page, least = 1, once = false) {
	await page.waitForFunction(
		(frames) => (window as unknown as { tops: unknown[] }).tops.length >= frames,
		FRAMES
	);
	const tops = await page.evaluate(() => (window as unknown as { tops: [number, number][] }).tops);
	const moves = tops.slice(1).flatMap(([top, shown], i) => {
		const step = Math.abs(top - tops[i][0]);
		return step >= 4 ? [{ step, shown: Math.min(shown, tops[i][1]) }] : [];
	});
	const trail = tops.map(([top, shown]) => `${Math.round(top)}@${shown.toFixed(2)}`).join(' ');
	expect(moves.length, trail).toBeGreaterThanOrEqual(least);
	expect(
		moves.filter(({ shown }) => shown > 0.4),
		trail
	).toEqual([]);
	const dims = tops.slice(1).filter(([, shown], i) => shown < 0.5 && tops[i][1] >= 0.5);
	if (once) expect(dims.length, trail).toBe(1);
}

/** Waits for `selector` to stand still on screen: motion turned off in the middle of a glide ends it in one frame. */
export async function restTop(page: Page, selector: string) {
	const top = () =>
		page.evaluate((s) => document.querySelector(s)?.getBoundingClientRect().top, selector);
	let last: number | undefined = NaN;
	await expect.poll(async () => last === (last = await top()), { intervals: [300] }).toBe(true);
}

type Press = { jumps: string[]; pops: string[]; twice: string[]; dims: number; frames: number };

/** Starts recording a press: what stands in the window and takes a new place while it shows (with motion allowed, in a step of
 * over `lone` px with nothing around it), what falls from showing to not twice, how many times the page dims and, inside `block`,
 * what comes or goes in one frame while it shows.
 * A fall is counted on each thing, its parents' opacity with it: the page's dims said once while the rows fell twice (pass 58). */
export async function watchPress(page: Page, block = '', lone = 8) {
	await page.evaluate(
		({ block, frames, lone }) => {
			const w = window as unknown as Press;
			type Place = {
				top: number;
				shown: number;
				name: string;
				step: number;
				before: number;
				gap: number;
			};
			const places = new Map<Element, Place>();
			const sliding = !matchMedia('(prefers-reduced-motion: reduce)').matches;
			const falls = new Map<Element, number>();
			w.jumps = [];
			w.pops = [];
			w.twice = [];
			w.dims = 0;
			w.frames = 0;
			let dim = false;
			let then = 0;
			const step = (time: number) => {
				// A frame that came late holds the steps of those it replaced: 31 px of a line's glide in one of 83 ms. Only that frame:
				// left out for 60 ms after it, a row's height down in a frame went unseen, a jump comes as the page has the most to do.
				const gap = time - then;
				then = time;
				const opacities = new Map<Element, number>();
				const seen = (e: Element | null): number => {
					if (!e) return 1;
					if (!opacities.has(e))
						opacities.set(e, Number(getComputedStyle(e).opacity) * seen(e.parentElement));
					return opacities.get(e)!;
				};
				const main = document.querySelector('main')!;
				const dark = Number(getComputedStyle(main).opacity) < 0.5;
				if (dark && !dim) w.dims += 1;
				dim = dark;
				const stands =
					'main :is(h1, h2, h3, p, li, label, input, select, button, td, summary, [data-whole])';
				const shown = new Set<Element>();
				for (const e of document.querySelectorAll(`${stands}${block ? `, ${block}` : ''}`)) {
					if (!e.checkVisibility()) continue;
					shown.add(e);
					const { top, bottom } = e.getBoundingClientRect();
					const name = `${e.tagName} ${(e.textContent ?? '').trim().slice(0, 28)}`;
					const now = seen(e);
					const was = places.get(e);
					const inSight = bottom > 0 && top < innerHeight;
					const step = was ? top - was.top : 0;
					if (!sliding && was && inSight && Math.abs(step) >= 4 && Math.min(now, was.shown) > 0.4)
						w.jumps.push(`${name}: ${Math.round(step)} px at ${now.toFixed(2)}`);
					// With motion allowed a glide takes many frames: what moves in one alone jumped (pass 62).
					const near = Math.abs(was?.step ?? 0) / 4;
					const alone =
						was &&
						was.gap <= 25 &&
						Math.abs(was.step) > lone &&
						Math.abs(was.before) < near &&
						Math.abs(step) < near;
					if (sliding && alone && inSight && Math.min(now, was.shown) > 0.4)
						w.jumps.push(
							`${name}: ${Math.round(was.step)} px alone in ${Math.round(was.gap)} ms at ${now.toFixed(2)}`
						);
					if (!sliding && was && inSight && was.shown >= 0.5 && now < 0.5) {
						falls.set(e, (falls.get(e) ?? 0) + 1);
						if (falls.get(e) === 2) w.twice.push(name);
					}
					// A fade that stayed at nothing and came in a frame: a fade under way read as what it should reach (pass 60).
					if (was && inSight && was.shown <= 0.1 && now >= 0.9)
						w.pops.push(`came in a frame: ${name}`);
					if (!was && w.frames && block && e.matches(block) && now > 0.4)
						w.pops.push(`in ${name} at ${now.toFixed(2)}`);
					places.set(e, { top, shown: now, name, step, before: was?.step ?? 0, gap });
				}
				for (const [e, was] of places) {
					if (shown.has(e)) continue;
					if (block && e.matches(block) && was.shown > 0.4)
						w.pops.push(`out ${was.name} at ${was.shown.toFixed(2)}`);
					places.delete(e);
				}
				if (++w.frames < frames) requestAnimationFrame(step);
			};
			requestAnimationFrame(step);
		},
		{ block, frames: FRAMES, lone }
	);
	await started(page);
}

/** Nothing took a new place while it showed, nothing came or went in a frame, and the page dimmed at most `most` times. */
export async function expectPress(page: Page, what: string, most = 1) {
	await page.waitForFunction((frames) => (window as unknown as Press).frames >= frames, FRAMES);
	const { jumps, pops, twice, dims } = await page.evaluate(() => {
		const { jumps, pops, twice, dims } = window as unknown as Press;
		return { jumps, pops, twice, dims };
	});
	expect({ what, jumps, pops, twice }).toEqual({ what, jumps: [], pops: [], twice: [] });
	expect(dims, `${what}: the page's dims`).toBeLessThanOrEqual(most);
}
