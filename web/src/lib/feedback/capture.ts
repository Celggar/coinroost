// Screenshots for a feedback report: the screen as it is now, or an image from the device. Both become a JPEG the API
// takes (at most 600,000 bytes, its long side at most 2,560 px). The screen is drawn by the page itself (snapdom):
// iOS Safari before 27 and Chrome on Android have no screen capture, and the drawing keeps the glass bars as the
// browser paints them (measured against browser screenshots, light, dark and scrolled). The library loads only when a
// screenshot is taken.

/** The API's cap for one image. */
export const MAX_BYTES = 600_000;
const LONG_SIDE = 2560;
const QUALITIES = [0.82, 0.7, 0.55, 0.4];

function jpeg(canvas: HTMLCanvasElement, quality: number): Promise<Blob | null> {
	return new Promise((resolve) => canvas.toBlob(resolve, 'image/jpeg', quality));
}

/** `source` drawn on a canvas no longer than the cap on its long side, at `scale` of that. */
function drawn(
	source: CanvasImageSource,
	width: number,
	height: number,
	scale = 1
): HTMLCanvasElement {
	const fit = Math.min(1, LONG_SIDE / Math.max(width, height)) * scale;
	const canvas = document.createElement('canvas');
	canvas.width = Math.max(1, Math.round(width * fit));
	canvas.height = Math.max(1, Math.round(height * fit));
	const context = canvas.getContext('2d');
	if (!context) throw new Error('no canvas');
	// A JPEG has no transparency: what was see-through lands on the page's own colour, not on black.
	context.fillStyle = getComputedStyle(document.body).backgroundColor || '#fff';
	context.fillRect(0, 0, canvas.width, canvas.height);
	context.drawImage(source, 0, 0, canvas.width, canvas.height);
	return canvas;
}

/** The smallest loss that fits the cap: lower quality first, then a smaller picture. */
async function fitted(source: CanvasImageSource, width: number, height: number): Promise<Blob> {
	for (const scale of [1, 0.75, 0.5]) {
		const canvas = drawn(source, width, height, scale);
		for (const quality of QUALITIES) {
			const blob = await jpeg(canvas, quality);
			if (blob && blob.size <= MAX_BYTES) return blob;
		}
	}
	throw new Error('too large');
}

/** What is on screen now, without what `leave` matches (the feedback panel itself). */
export async function captureScreen(leave: string): Promise<Blob> {
	const { snapdom } = await import('@zumer/snapdom');
	const shot = await snapdom(document.documentElement, { clip: 'viewport', exclude: [leave] });
	const canvas = await shot.toCanvas();
	return fitted(canvas, canvas.width, canvas.height);
}

/** An image from the device, made a JPEG the API takes. A file that is not an image a browser can read is refused. */
export async function fromFile(file: File): Promise<Blob> {
	const bitmap = await createImageBitmap(file);
	try {
		return await fitted(bitmap, bitmap.width, bitmap.height);
	} finally {
		bitmap.close();
	}
}

/** The bytes of `blob` in base64, as the API takes an image. */
export async function base64(blob: Blob): Promise<string> {
	const bytes = new Uint8Array(await blob.arrayBuffer());
	let text = '';
	for (let i = 0; i < bytes.length; i += 0x8000) {
		text += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
	}
	return btoa(text);
}
