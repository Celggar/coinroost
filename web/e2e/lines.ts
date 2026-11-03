// How a block of text wraps, measured from its words' boxes.
import type { Locator, Page } from '@playwright/test';

/** Whether the last word of `target` sits alone on its own line (a hard space counts as a space). */
export async function endsAlone(target: Locator): Promise<boolean> {
	return target.evaluate((element) => {
		const words: Range[] = [];
		const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
		while (walker.nextNode()) {
			const node = walker.currentNode as Text;
			for (const match of node.data.matchAll(/\S+/g)) {
				const range = document.createRange();
				range.setStart(node, match.index);
				range.setEnd(node, match.index + match[0].length);
				words.push(range);
			}
		}
		if (words.length < 2) return false;
		const top = (range: Range) => range.getClientRects()[0].top;
		return Math.abs(top(words.at(-1)!) - top(words.at(-2)!)) > 4;
	});
}

/** How many lines the text directly inside `target` takes (its child elements left out). */
export async function ownLines(target: Locator): Promise<number> {
	return target.evaluate((element) => {
		const tops = new Set<number>();
		for (const node of element.childNodes) {
			if (node.nodeType !== Node.TEXT_NODE || !node.textContent?.trim()) continue;
			const range = document.createRange();
			range.selectNodeContents(node);
			for (const rect of range.getClientRects()) if (rect.width > 0) tops.add(Math.round(rect.top));
		}
		return tops.size;
	});
}

/** Each tab's label with the widest count («99+») on every tab: the lines its own text takes, and how many whole
 * pixels it runs into the tab's padding. */
export async function widestTabs(page: Page): Promise<{ lines: number; spill: number }[]> {
	const nav = page.locator('nav.tabs');
	await nav.evaluate((element) => {
		const badge = element.querySelector('.badge');
		for (const name of element.querySelectorAll('.name')) {
			const own =
				name.querySelector('.badge') ?? name.appendChild(badge!.cloneNode(true) as Element);
			own.querySelector('span')!.textContent = '99+';
		}
	});
	const tabs = [];
	for (const tab of await nav.locator('a').all()) {
		const spill = await tab.evaluate((a) => {
			const name = a.querySelector('.name')!.getBoundingClientRect();
			const inner = a.getBoundingClientRect().right - parseFloat(getComputedStyle(a).paddingRight);
			return Math.max(0, Math.round(name.right - inner));
		});
		tabs.push({ lines: await ownLines(tab.locator('.name')), spill });
	}
	return tabs;
}
