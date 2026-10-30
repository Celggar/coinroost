<script lang="ts" module>
	/** A phone, where the lead text says to tap the same-device link instead of scanning. */
	export const onPhone = /iPhone|iPod|Android/.test(navigator.userAgent);
</script>

<script lang="ts">
	// What an authenticator app needs to add a code: the QR, the key to type or copy, and on Apple devices and
	// Android a link that opens the app on this device. Used by the setup link's second step and by the new
	// authenticator after a recovery code.
	import { t } from '#lib/i18n.svelte.ts';
	import { pop, vanish } from '#lib/motion.ts';

	let { enrollment }: { enrollment: { uri: string; secret: string; qr_svg: string } } = $props();

	const QR_SIZE = 212;
	const agent = navigator.userAgent;
	let qrPng = $state('');
	let copied = $state(false);
	let copiedTimer: ReturnType<typeof setTimeout> | undefined;

	const qr = $derived(`data:image/svg+xml;base64,${btoa(enrollment.qr_svg)}`);
	const grouped = $derived(enrollment.secret.match(/.{1,4}/g)?.join(' ') ?? '');
	// The same-device link: Apple's own scheme on Apple devices (iOS 15, macOS 12 and later), the common one on
	// Android, where authenticator apps register it, and none on other desktops, where nothing handles it.
	const sameDevice = $derived.by(() => {
		if (/iPhone|iPad|iPod|Macintosh/.test(agent))
			return enrollment.uri.replace(/^otpauth:/, 'apple-otpauth:');
		return /Android/.test(agent) ? enrollment.uri : '';
	});

	$effect(() => {
		const svg = enrollment.qr_svg;
		rasterize(svg)
			.then((png) => {
				if (svg === enrollment.qr_svg) qrPng = png;
			})
			.catch(() => (qrPng = ''));
	});

	/** The server's QR as a PNG: Safari offers to add a long-pressed QR to Passwords only for raster images (WWDC21
	 * 10105). Whole device pixels per module keep it sharp; what is left over widens the white border. Where the
	 * screen's scale is fractional the image is scaled by a fraction of a pixel, so it is drawn pixelated (CSS). */
	async function rasterize(svg: string): Promise<string> {
		const modules = Number(/viewBox="0 0 (\d+)/.exec(svg)?.[1] ?? 0);
		const image = new Image();
		image.src = `data:image/svg+xml;base64,${btoa(svg)}`;
		await image.decode();
		const canvas = document.createElement('canvas');
		canvas.width = canvas.height = Math.round(QR_SIZE * Math.max(2, devicePixelRatio));
		const context = canvas.getContext('2d');
		if (!context || !modules) return '';
		const scale = Math.floor(canvas.width / modules);
		const offset = Math.floor((canvas.width - modules * scale) / 2);
		context.fillStyle = '#ffffff';
		context.fillRect(0, 0, canvas.width, canvas.height);
		context.imageSmoothingEnabled = false;
		context.drawImage(image, offset, offset, modules * scale, modules * scale);
		return canvas.toDataURL('image/png');
	}

	async function copy() {
		await navigator.clipboard.writeText(enrollment.secret);
		copied = true;
		clearTimeout(copiedTimer);
		copiedTimer = setTimeout(() => (copied = false), 2000);
	}
</script>

<div class="qr" in:pop|global={{ delay: 150, start: 0.85 }}>
	<img src={qrPng || qr} alt={t('setup.qr_alt')} width={QR_SIZE} height={QR_SIZE} />
</div>
<div class="key">
	<span class="key-label">{t('setup.key')}</span>
	<div class="key-box">
		<code data-testid="setup-key">{grouped}</code>
		<button type="button" class="copy" aria-label={t('setup.copy')} onclick={copy}>
			{#key copied}
				<span in:pop={{ start: 0.8 }} out:vanish>
					{copied ? t('setup.copied_short') : t('setup.copy_short')}
				</span>
			{/key}
		</button>
	</div>
	<span class="visually-hidden" aria-live="polite">{copied ? t('setup.copied') : ''}</span>
	{#if sameDevice}<a class="open" href={sameDevice}>{t('setup.open_app')}</a>{/if}
</div>

<style>
	.qr {
		justify-self: center;
		padding: 8px;
		border-radius: 16px;
		background: #ffffff;
		border: 1px solid #d0d6d8;
		line-height: 0;
	}
	.qr img {
		display: block;
		width: 212px;
		height: 212px;
		image-rendering: pixelated;
	}
	.key {
		display: grid;
		gap: 6px;
	}
	.key-label {
		font-weight: 600;
	}
	.key-box {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		min-height: 48px;
		padding: 0 4px 0 12px;
		border: 1px solid var(--field-border);
		border-radius: 10px;
		background: var(--surface);
	}
	code {
		font-family:
			ui-monospace, SFMono-Regular, Menlo, Consolas, 'Liberation Mono', 'DejaVu Sans Mono',
			monospace;
		font-size: 1rem;
		letter-spacing: 0.04em;
		overflow-wrap: anywhere;
		user-select: all;
	}
	.copy {
		flex: none;
		display: grid;
		place-items: center;
		min-height: 44px;
		padding: 0 12px;
		border: 0;
		border-radius: 8px;
		background: color-mix(in oklab, var(--accent) 12%, transparent);
		color: var(--link);
		font-weight: 600;
		--state-layer: color-mix(in oklab, var(--accent) 10%, transparent);
	}
	.copy > :global(*) {
		grid-area: 1 / 1;
	}
	/* A 44 px tall target (Apple's minimum) without moving anything around it. */
	.open {
		font-weight: 600;
		text-underline-offset: 3px;
		justify-self: start;
		padding-block: 10px;
		margin-block: -10px;
	}
</style>
