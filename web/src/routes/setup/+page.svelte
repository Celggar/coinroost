<script lang="ts">
	// A single-use setup link: the token is the part after `#`, which the browser never sends in a request; the
	// API gets it in request bodies only. Step 1 asks for the email and password, step 2 shows the authenticator
	// secret the server made for this link and checks a code with it, step 3 shows the recovery codes the new user
	// got (once); then the person is in. Steps slide in from
	// the side they come from, and the card grows or shrinks to the new step (the shell's job). With no link, a
	// fresh install that allows it hands out one for its first account.
	import { goto, replaceState } from '$app/navigation';
	import { onMount, tick } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import AuthShell from '#lib/auth/AuthShell.svelte';
	import Authenticator, { onPhone } from '#lib/auth/Authenticator.svelte';
	import CodeField from '#lib/auth/CodeField.svelte';
	import FormAlert from '#lib/auth/FormAlert.svelte';
	import PasswordField from '#lib/auth/PasswordField.svelte';
	import RecoveryCodes from '#lib/auth/RecoveryCodes.svelte';
	import { t } from '#lib/i18n.svelte.ts';
	import { DURATION, pop, stepIn, stepOut, vanish } from '#lib/motion.ts';
	import { refresh } from '#lib/session.svelte.ts';

	type Step = 'checking' | 'invalid' | 'account' | 'code' | 'codes';
	const order: Step[] = ['checking', 'invalid', 'account', 'code', 'codes'];

	let step = $state<Step>('checking');
	let direction = $state(1);
	let token = '';
	let email = $state('');
	let password = $state('');
	let code = $state('');
	let enrollment = $state<Schemas['InviteEnrollment'] | null>(null);
	let first = $state(false);
	let recovery = $state<{ codes: string[]; createdAt: string }>({ codes: [], createdAt: '' });
	let error = $state({ text: '', n: 0 });
	let busy = $state(false);
	let passwordField: PasswordField | undefined = $state();
	let codeField: CodeField | undefined = $state();
	let introEnded: (() => void) | undefined;

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
		const {
			data,
			error: failure,
			response
		} = await api.POST('/api/v1/invites/accept', {
			body: { token, email, password, code }
		});
		busy = false;
		if (data) {
			// The link is spent: a reload now shows it as used, and the codes can be made again in Security.
			recovery = { codes: data.recovery_codes, createdAt: new Date().toISOString() };
			error = { text: '', n: error.n + 1 };
			go('codes');
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

	async function enter() {
		await refresh();
		await goto('/', { replaceState: true });
	}

	function back() {
		error = { text: '', n: error.n + 1 };
		go('account');
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
					{:else if step === 'code' && enrollment}
						<form class="stack" onsubmit={finish}>
							<div>
								<h2 class="step-label">{t('setup.step_code')}</h2>
								<p class="lead">{onPhone ? t('setup.scan_phone') : t('setup.scan')}</p>
							</div>
							<Authenticator {enrollment} />
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
					{:else if step === 'codes'}
						<div class="stack">
							<div>
								<h2 class="step-label">{t('setup.step_codes')}</h2>
								<p class="lead">{t('setup.codes_lead')}</p>
							</div>
							<RecoveryCodes codes={recovery.codes} {email} createdAt={recovery.createdAt} />
							<div class="actions">
								<button type="button" class="primary" onclick={enter} data-flip
									>{t('setup.codes_done')}</button
								>
							</div>
						</div>
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
