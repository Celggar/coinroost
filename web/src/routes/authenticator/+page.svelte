<script lang="ts">
	// After a login with a recovery code the old authenticator is retired, and the layout keeps the person here until
	// a new one is set up: the server makes its secret, the person adds it to their app and sends a code from it.
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import AuthShell from '#lib/auth/AuthShell.svelte';
	import Authenticator, { onPhone } from '#lib/auth/Authenticator.svelte';
	import CodeField from '#lib/auth/CodeField.svelte';
	import FormAlert from '#lib/auth/FormAlert.svelte';
	import { t } from '#lib/i18n.svelte.ts';
	import { pop, vanish } from '#lib/motion.ts';
	import { logOut, refresh, session } from '#lib/session.svelte.ts';

	let enrollment = $state<Schemas['NewAuthenticator'] | null>(null);
	let code = $state('');
	let error = $state({ text: '', n: 0 });
	let busy = $state(false);
	let codeField: CodeField | undefined = $state();

	onMount(async () => {
		const { data, error: failure, response } = await api.POST('/api/v1/me/authenticator');
		if (response.status === 409) {
			await goto('/', { replaceState: true });
		} else if (data) {
			enrollment = data;
		} else {
			error = {
				text: problem(failure, response.status, t('authenticator.failed')),
				n: error.n + 1
			};
		}
	});

	async function finish(event: SubmitEvent) {
		event.preventDefault();
		if (busy) return;
		busy = true;
		const { error: failure, response } = await api.POST('/api/v1/me/authenticator/confirm', {
			body: { code }
		});
		busy = false;
		if (response.ok) {
			await refresh();
			await goto('/security', { replaceState: true });
			return;
		}
		code = '';
		const text =
			response.status === 422
				? t('setup.wrong_code')
				: problem(failure, response.status, t('authenticator.failed'));
		error = { text, n: error.n + 1 };
		codeField?.shake();
	}

	async function leave() {
		await logOut();
		await goto('/login');
	}
</script>

<svelte:head>
	<title>{t('app.title', { page: t('authenticator.title') })}</title>
</svelte:head>

<AuthShell compact>
	<form class="stack" onsubmit={finish}>
		<div>
			<h1>{t('authenticator.title')}</h1>
			<p class="lead">{t('authenticator.lead')}</p>
		</div>
		{#if enrollment}
			<p class="lead">{onPhone ? t('setup.scan_phone') : t('setup.scan')}</p>
			<Authenticator {enrollment} />
			<hr />
			<!-- The email, hidden, so a password manager saves the code with the right account. -->
			<input
				type="email"
				autocomplete="username"
				value={session.user?.email ?? ''}
				hidden
				readonly
			/>
			<CodeField
				bind:this={codeField}
				label={t('login.code')}
				hint={t('setup.code_hint')}
				bind:value={code}
			/>
		{/if}
		<div class="actions">
			<FormAlert {...error} />
			{#if enrollment}
				<button type="submit" class="primary" aria-disabled={busy} data-flip>
					{#key busy}
						<span in:pop={{ start: 0.9 }} out:vanish>
							{busy ? t('authenticator.busy') : t('authenticator.finish')}
						</span>
					{/key}
				</button>
			{/if}
		</div>
		<button type="button" class="back" onclick={leave} data-flip>{t('nav.log_out')}</button>
	</form>
</AuthShell>

<style>
	hr {
		width: 100%;
		margin: 0;
		border: 0;
		border-top: 1px solid color-mix(in oklab, var(--field-border) 35%, transparent);
	}
	.back {
		justify-self: center;
		min-height: 44px;
		border: 0;
		background: transparent;
		color: var(--link);
		text-decoration: underline;
		text-underline-offset: 3px;
		--state-layer: transparent;
	}
</style>
