<script lang="ts">
	// A single-use setup link: the token is the part after `#`, which the browser never sends in a request; the
	// API gets it in request bodies only. Step 1 asks for the email and password, step 2 shows the authenticator
	// secret the server made for this link and checks a code with it; then the person is in. Steps slide in from
	// the side they come from, and the card grows or shrinks to the new step (the shell's job). With no link, a
	// fresh install that allows it hands out one for its first account.
	import { goto, replaceState } from '$app/navigation';
	import { onMount, tick } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import AuthShell from '#lib/auth/AuthShell.svelte';
	import CodeField from '#lib/auth/CodeField.svelte';
	import FormAlert from '#lib/auth/FormAlert.svelte';
	import PasswordField from '#lib/auth/PasswordField.svelte';
	import { t } from '#lib/i18n.svelte.ts';
	import { DURATION, pop, stepIn, stepOut, vanish } from '#lib/motion.ts';
	import { refresh } from '#lib/session.svelte.ts';

	type Step = 'checking' | 'invalid' | 'account' | 'code';
	const order: Step[] = ['checking', 'invalid', 'account', 'code'];

	let step = $state<Step>('checking');
	let direction = $state(1);
	let token = '';
	let email = $state('');
	let password = $state('');
	let code = $state('');
	let enrollment = $state<Schemas['InviteEnrollment'] | null>(null);
	let first = $state(false);
	let qrPng = $state('');
	let error = $state({ text: '', n: 0 });
	let busy = $state(false);
	let copied = $state(false);
	let copiedTimer: ReturnType<typeof setTimeout> | undefined;
	let passwordField: PasswordField | undefined = $state();
	let codeField: CodeField | undefined = $state();
	let introEnded: (() => void) | undefined;

	const QR_SIZE = 212;
	const qr = $derived(enrollment ? `data:image/svg+xml;base64,${btoa(enrollment.qr_svg)}` : '');
	const grouped = $derived(enrollment?.secret.match(/.{1,4}/g)?.join(' ') ?? '');
	const agent = navigator.userAgent;
	const onPhone = /iPhone|iPod|Android/.test(agent);
	// The same-device link: Apple's own scheme on Apple devices (iOS 15, macOS 12 and later), the common one on
	// Android, where authenticator apps register it, and none on other desktops, where nothing handles it.
	const sameDevice = $derived.by(() => {
		if (!enrollment) return '';
		if (/iPhone|iPad|iPod|Macintosh/.test(agent))
			return enrollment.uri.replace(/^otpauth:/, 'apple-otpauth:');
		return /Android/.test(agent) ? enrollment.uri : '';
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

	/** A fresh install's first account: the server hands out a setup link, kept in the address so a reload finds it. */
	async function firstAccountLink(): Promise<string> {
		const { data: state } = await api.GET('/api/v1/first-account');
		if (!state?.open) return '';
		const { data } = await api.POST('/api/v1/first-account');
		if (!data) return '';
		replaceState(`#${encodeURIComponent(data.token)}`, {});
		return data.token;
	}

	function go(next: Step) {
		direction = order.indexOf(next) >= order.indexOf(step) ? 1 : -1;
		step = next;
	}

	function fail(text: string) {
		error = { text, n: error.n + 1 };
	}

	/** Back to an earlier step with a message: the step's alert region is on the page, empty, before the message
	 * lands in it, so screen readers announce it. */
	async function backWith(next: Step, text: string) {
		error = { text: '', n: error.n + 1 };
		const arrived = new Promise<void>((resolve) => (introEnded = resolve));
		go(next);
		await tick();
		await Promise.race([
			arrived,
			new Promise((resolve) => setTimeout(resolve, DURATION.medium * 2))
		]);
		introEnded = undefined;
		fail(text);
	}

	onMount(async () => {
		token = decodeURIComponent(location.hash.slice(1)) || (await firstAccountLink());
		if (!token) {
			go('invalid');
			return;
		}
		const { data } = await api.POST('/api/v1/invites/check', { body: { token } });
		first = data?.first_account ?? false;
		go(data ? 'account' : 'invalid');
	});

	async function toCode(event: SubmitEvent) {
		event.preventDefault();
		if (busy) return;
		passwordField?.hide();
		busy = true;
		const {
			data,
			error: failure,
			response
		} = await api.POST('/api/v1/invites/enrollment', {
			body: { token, email }
		});
		busy = false;
		if (response.status === 404) {
			go('invalid');
		} else if (data) {
			qrPng = await rasterize(data.qr_svg).catch(() => '');
			enrollment = data;
			error = { text: '', n: error.n + 1 };
			go('code');
		} else {
			fail(problem(failure, response.status, t('setup.failed')));
		}
	}

	async function finish(event: SubmitEvent) {
		event.preventDefault();
		if (busy) return;
		busy = true;
		const { error: failure, response } = await api.POST('/api/v1/invites/accept', {
			body: { token, email, password, code }
		});
		busy = false;
		if (response.ok) {
			await refresh();
			await goto('/', { replaceState: true });
			return;
		}
		code = '';
		if (response.status === 404) {
			go('invalid');
		} else if (response.status === 409) {
			await backWith('account', t('setup.taken'));
		} else if (response.status === 422) {
			fail(t('setup.wrong_code'));
			codeField?.shake();
		} else {
			fail(problem(failure, response.status, t('setup.failed')));
		}
	}

	function back() {
		error = { text: '', n: error.n + 1 };
		go('account');
	}

	async function copy() {
		await navigator.clipboard.writeText(enrollment?.secret ?? '');
		copied = true;
		clearTimeout(copiedTimer);
		copiedTimer = setTimeout(() => (copied = false), 2000);
	}
</script>

<svelte:head>
	<title>{t('app.title', { page: t('setup.title') })}</title>
</svelte:head>

<AuthShell compact>
	<h1>{t('setup.title')}</h1>
	<div class="steps">
		<div class="cell">
			{#key step}
				<div
					class="step"
					in:stepIn={{ direction }}
					out:stepOut={{ direction }}
					onintroend={() => introEnded?.()}
				>
					{#if step === 'checking'}
						<p class="checking">
							<span class="spinner" aria-hidden="true"></span>{t('setup.checking')}
						</p>
					{:else if step === 'invalid'}
						<h2 class="step-label">{t('setup.invalid_title')}</h2>
						<p class="lead">{t('setup.invalid')}</p>
					{:else if step === 'account'}
						<form class="stack" onsubmit={toCode}>
							<div>
								<h2 class="step-label">{t('setup.step_account')}</h2>
								{#if first}<p class="lead">{t('setup.first_lead')}</p>{/if}
							</div>
							<label>
								{t('login.email')}
								<input type="email" autocomplete="username" required bind:value={email} />
							</label>
							<PasswordField
								bind:this={passwordField}
								label={t('login.password')}
								hint={t('setup.password_hint')}
								autocomplete="new-password"
								minlength={12}
								bind:value={password}
							/>
							<div class="actions">
								<FormAlert {...error} />
								<button type="submit" class="primary" aria-disabled={busy} data-flip
									>{t('setup.continue')}</button
								>
							</div>
						</form>
					{:else if enrollment}
						<form class="stack" onsubmit={finish}>
							<div>
								<h2 class="step-label">{t('setup.step_code')}</h2>
								<p class="lead">{onPhone ? t('setup.scan_phone') : t('setup.scan')}</p>
							</div>
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
								<span class="visually-hidden" aria-live="polite"
									>{copied ? t('setup.copied') : ''}</span
								>
								{#if sameDevice}<a class="open" href={sameDevice}>{t('setup.open_app')}</a>{/if}
							</div>
							<hr />
							<!-- The email again, hidden, so a password manager saves the code with the right account. -->
							<input type="email" autocomplete="username" value={email} hidden readonly />
							<CodeField
								bind:this={codeField}
								label={t('login.code')}
								hint={t('setup.code_hint')}
								bind:value={code}
							/>
							<div class="actions">
								<FormAlert {...error} />
								<button type="submit" class="primary" aria-disabled={busy} data-flip>
									{#key busy}
										<span in:pop={{ start: 0.9 }} out:vanish>
											{busy ? t('setup.busy') : t('setup.finish')}
										</span>
									{/key}
								</button>
							</div>
							<button type="button" class="back" onclick={back} data-flip>{t('setup.back')}</button>
						</form>
					{/if}
				</div>
			{/key}
		</div>
	</div>
</AuthShell>

<style>
	/* The step label sits right under the title, as in the design. */
	.steps {
		margin-top: -20px;
	}
	.cell {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
	}
	.step {
		grid-area: 1 / 1;
		min-width: 0;
	}
	.checking {
		display: flex;
		align-items: center;
		gap: 10px;
		margin: 6px 0 0;
		color: var(--muted);
	}
	.spinner {
		width: 18px;
		height: 18px;
		border-radius: 50%;
		border: 2px solid var(--field-border);
		border-top-color: var(--accent);
	}
	@media (prefers-reduced-motion: no-preference) {
		.spinner {
			animation: spin 800ms linear infinite;
		}
	}
	@keyframes spin {
		to {
			transform: rotate(1turn);
		}
	}
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
		min-height: 40px;
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
	hr {
		width: 100%;
		margin: 0;
		border: 0;
		border-top: 1px solid color-mix(in oklab, var(--field-border) 35%, transparent);
	}
	.back {
		justify-self: center;
		border: 0;
		background: transparent;
		color: var(--link);
		text-decoration: underline;
		text-underline-offset: 3px;
		--state-layer: transparent;
	}
</style>
