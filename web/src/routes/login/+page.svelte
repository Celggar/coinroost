<script lang="ts">
	// The login: email, password and the authenticator's code, or, for a lost phone, one of the recovery codes. A
	// recovery code leads to setting up a new authenticator before anything else.
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api, problem } from '#lib/api/client.ts';
	import AuthShell from '#lib/auth/AuthShell.svelte';
	import CodeField from '#lib/auth/CodeField.svelte';
	import FormAlert from '#lib/auth/FormAlert.svelte';
	import PasswordField from '#lib/auth/PasswordField.svelte';
	import { t } from '#lib/i18n.svelte.ts';
	import { pop, stepIn, stepOut, vanish } from '#lib/motion.ts';
	import { refresh, session } from '#lib/session.svelte.ts';

	let email = $state('');
	let password = $state('');
	let code = $state('');
	let recoveryCode = $state('');
	let recovering = $state(false);
	let error = $state({ text: '', n: 0 });
	let busy = $state(false);
	let passwordField: PasswordField | undefined = $state();
	let codeField: CodeField | undefined = $state();

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (busy) return;
		passwordField?.hide();
		busy = true;
		const second = recovering ? { recovery_code: recoveryCode } : { code };
		const { error: failure, response } = await api.POST('/api/v1/sessions', {
			body: { email, password, ...second }
		});
		busy = false;
		if (failure || !response.ok) {
			const wrong = recovering ? t('login.recovery_wrong') : t('login.wrong');
			const text =
				response?.status === 401 ? wrong : problem(failure, response?.status, t('login.failed'));
			error = { text, n: error.n + 1 };
			code = '';
			codeField?.shake();
			return;
		}
		await refresh();
		if (session.user?.reenroll_required) {
			await goto('/authenticator', { replaceState: true });
			return;
		}
		const next = page.url.searchParams.get('next');
		await goto(next?.startsWith('/') && !next.startsWith('//') ? next : '/');
	}

	function switchSecond() {
		recovering = !recovering;
		error = { text: '', n: error.n + 1 };
	}
</script>

<svelte:head>
	<title>{t('app.title', { page: t('login.title') })}</title>
</svelte:head>

<AuthShell>
	<div>
		<h1>{t('login.title')}</h1>
		<p class="lead">{t('login.subtitle')}</p>
	</div>
	<form class="stack" onsubmit={submit}>
		<label>
			{t('login.email')}
			<input type="email" autocomplete="username" required bind:value={email} />
		</label>
		<PasswordField
			bind:this={passwordField}
			label={t('login.password')}
			autocomplete="current-password"
			bind:value={password}
		/>
		<div class="second">
			{#key recovering}
				<div
					class="swap"
					in:stepIn={{ direction: recovering ? 1 : -1 }}
					out:stepOut={{ direction: recovering ? 1 : -1 }}
				>
					{#if recovering}
						<label>
							{t('login.recovery_code')}
							<span class="hint">{t('login.recovery_hint')}</span>
							<input
								type="text"
								autocomplete="off"
								autocapitalize="none"
								spellcheck="false"
								required
								bind:value={recoveryCode}
							/>
						</label>
					{:else}
						<CodeField
							bind:this={codeField}
							label={t('login.code')}
							hint={t('login.code_hint')}
							bind:value={code}
						/>
					{/if}
					<button type="button" class="switch" onclick={switchSecond} data-flip>
						{recovering ? t('login.use_code') : t('login.use_recovery')}
					</button>
				</div>
			{/key}
		</div>
		<div class="actions">
			<FormAlert {...error} />
			<button type="submit" class="primary" aria-disabled={busy} data-flip>
				{#key busy}
					<span in:pop={{ start: 0.9 }} out:vanish>{busy ? t('login.busy') : t('login.title')}</span
					>
				{/key}
			</button>
		</div>
	</form>
</AuthShell>

<style>
	/* The two second steps take turns in one cell, sliding like the setup steps. */
	.second {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
	}
	.swap {
		grid-area: 1 / 1;
		display: grid;
		gap: 10px;
		min-width: 0;
	}
	.switch {
		justify-self: start;
		min-height: 44px;
		padding: 0;
		border: 0;
		background: transparent;
		color: var(--link);
		text-decoration: underline;
		text-underline-offset: 3px;
		--state-layer: transparent;
	}
</style>
