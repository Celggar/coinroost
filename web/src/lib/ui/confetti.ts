// A short burst for a milestone a person reached by hand: a month imported, the queue sorted to the end. Rare on
// purpose: once per milestone per visit, 1.5 s at most, nothing behind it to wait for, and never for what the system did
// or where motion or colours are asked to stay still. It is decoration, so screen readers and pointers pass through.
const COLOURS = ['--accent', '--ok', '--warn', '--link'];
const PIECES = 28;
const LONGEST = 1400;

const celebrated = new Set<string>();

export function confetti(milestone: 'imported' | 'sorted') {
	if (celebrated.has(milestone)) return;
	celebrated.add(milestone);
	if (matchMedia('(prefers-reduced-motion: reduce), (forced-colors: active)').matches) return;
	const box = document.createElement('div');
	box.setAttribute('aria-hidden', 'true');
	Object.assign(box.style, {
		position: 'fixed',
		inset: '0',
		zIndex: '40',
		overflow: 'hidden',
		pointerEvents: 'none'
	});
	const motions = Array.from({ length: PIECES }, (_, i) => {
		const piece = document.createElement('span');
		Object.assign(piece.style, {
			position: 'absolute',
			top: '-16px',
			left: `${8 + Math.random() * 84}%`,
			width: '8px',
			height: '12px',
			borderRadius: '2px',
			background: `var(${COLOURS[i % COLOURS.length]})`
		});
		box.append(piece);
		const fall = innerHeight * (0.55 + Math.random() * 0.35);
		const drift = (Math.random() - 0.5) * 160;
		const turn = (Math.random() - 0.5) * 720;
		return piece.animate(
			[
				{ transform: 'translate(0, 0) rotate(0deg)', opacity: 1 },
				{ transform: `translate(${drift}px, ${fall}px) rotate(${turn}deg)`, opacity: 0 }
			],
			{
				duration: LONGEST - Math.random() * 400,
				delay: Math.random() * 100,
				easing: 'cubic-bezier(0.2, 0.6, 0.4, 1)',
				fill: 'forwards'
			}
		);
	});
	document.body.append(box);
	Promise.all(motions.map((motion) => motion.finished.catch(() => undefined))).then(() =>
		box.remove()
	);
}
