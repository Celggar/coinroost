// A table box that scrolls sideways has to be reachable with the keyboard, or its hidden columns are out of reach
// for people who do not use a pointer (WCAG 2.1.1; axe `scrollable-region-focusable`). It takes the focus, as a
// named region, only while its table is wider than it, so a table that fits adds no stop to the tab order.
export function scrollable(node: HTMLElement, label: string) {
	const update = () => {
		if (node.scrollWidth > node.clientWidth + 1) {
			node.tabIndex = 0;
			node.setAttribute('role', 'region');
			node.setAttribute('aria-label', label);
		} else {
			node.removeAttribute('tabindex');
			node.removeAttribute('role');
			node.removeAttribute('aria-label');
		}
	};
	const observer = new ResizeObserver(update);
	observer.observe(node);
	if (node.firstElementChild) observer.observe(node.firstElementChild);
	update();
	return {
		update(next: string) {
			label = next;
			update();
		},
		destroy: () => observer.disconnect()
	};
}
