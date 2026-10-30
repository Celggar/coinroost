<script lang="ts">
	// A single-use setup link: the token is the part after `#`, which the browser never sends in a request; the
	// API gets it in request bodies only. Step 1 asks for the email and password, step 2 shows the authenticator
	// secret the server made for this link and checks a code with it; then the person is in.
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import LanguageSwitch from '#lib/LanguageSwitch.svelte';
	import { t } from '#lib/i18n.svelte.ts';
	import { refresh } from '#lib/session.svelte.ts';

	type Step = 'checking' | 'invalid' | 'account' | 'code';

	let step = $state<Step>('checking');
	let token = '';
	let email = $state('');
	let password = $state('');
	let code = $state('');
	let enrollment = $state<Schemas['InviteEnrollment'] | null>(null);
	let error = $state('');
	let busy = $state(false);
	let copied = $state(false);

	const qr = $derived(enrollment ? `data:image/svg+xml;base64,${btoa(enrollment.qr_svg)}` : '');
	const grouped = $derived(enrollment?.secret.match(/.{1,4}/g)?.join(' ') ?? '');

	onMount(async () => {
		token = decodeURIComponent(location.hash.slice(1));
		if (!token) {
			step = 'invalid';
			return;
		}
		const { response } = await api.POST('/api/v1/invites/check', { body: { token } });
		step = response.ok ? 'account' : 'invalid';
	});

	async function toCode(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = '';
		const {
			data,
			error: failure,
			response
		} = await api.POST('/api/v1/invites/enrollment', {
			body: { token, email }
		});
		busy = false;
		if (response.status === 404) {
			step = 'invalid';
		} else if (data) {
			enrollment = data;
			step = 'code';
		} else {
			error = problem(failure, response.status, t('setup.failed'));
		}
	}

	async function finish(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = '';
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
			step = 'invalid';
		} else if (response.status === 409) {
			error = t('setup.taken');
			step = 'account';
		} else if (response.status === 422) {
			error = t('setup.wrong_code');
		} else {
			error = problem(failure, response.status, t('setup.failed'));
		}
	}

	async function copy() {
		await navigator.clipboard.writeText(enrollment?.secret ?? '');
		copied = true;
	}
</script>

<svelte:head>
	<title>{t('app.title', { page: t('setup.title') })}</title>
</svelte:head>

<section class="setup card">
	<div class="heading">
		<h1>{t('setup.title')}</h1>
		<LanguageSwitch />
	</div>

	{#if step === 'checking'}
		<p class="hint">{t('setup.checking')}</p>
	{:else if step === 'invalid'}
		<h2>{t('setup.invalid_title')}</h2>
		<p>{t('setup.invalid')}</p>
	{:else if step === 'account'}
		<form class="stack" onsubmit={toCode}>
			<h2>{t('setup.step_account')}</h2>
			<label>
				{t('login.email')}
				<input type="email" autocomplete="username" required bind:value={email} />
			</label>
			<label>
				{t('login.password')}
				<span class="hint">{t('setup.password_hint')}</span>
				<input
					type="password"
					autocomplete="new-password"
					minlength="12"
					required
					bind:value={password}
				/>
			</label>
			{#if error}<p class="alert" role="alert">{error}</p>{/if}
			<button type="submit" disabled={busy}>{t('setup.continue')}</button>
		</form>
	{:else if enrollment}
		<form class="stack" onsubmit={finish}>
			<h2>{t('setup.step_code')}</h2>
			<p>{t('setup.scan')}</p>
			<img class="qr" src={qr} alt={t('setup.qr_alt')} width="200" height="200" />
			<div class="key">
				<span class="hint">{t('setup.key')}</span>
				<code data-testid="setup-key">{grouped}</code>
				<button type="button" class="secondary" onclick={copy}>
					{copied ? t('setup.copied') : t('setup.copy')}
				</button>
				<a href={enrollment.uri}>{t('setup.open_app')}</a>
			</div>
			<!-- The email again, hidden, so a password manager saves the code with the right account. -->
			<input type="email" autocomplete="username" value={email} hidden readonly />
			<label>
				{t('login.code')}
				<span class="hint">{t('setup.code_hint')}</span>
				<input
					inputmode="numeric"
					autocomplete="one-time-code"
					pattern={'[0-9]{6}'}
					maxlength="6"
					required
					bind:value={code}
				/>
			</label>
			{#if error}<p class="alert" role="alert">{error}</p>{/if}
			<button type="submit" disabled={busy}>{busy ? t('setup.busy') : t('setup.finish')}</button>
			<button type="button" class="secondary" onclick={() => (step = 'account')}>
				{t('setup.back')}
			</button>
		</form>
	{/if}
</section>

<style>
	.setup {
		max-width: 460px;
		margin: 8vh auto 0;
	}
	.heading {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
	}
	h1 {
		margin: 0;
	}
	h2 {
		font-size: 1rem;
		margin: 16px 0 0;
	}
	.qr {
		align-self: center;
		background: #fff;
		border-radius: var(--radius);
	}
	.key {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px;
	}
	.key .hint {
		flex-basis: 100%;
	}
	code {
		font-size: 1.05rem;
		letter-spacing: 0.04em;
		word-break: break-all;
	}
</style>
