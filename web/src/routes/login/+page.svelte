<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api, problem } from '#lib/api/client.ts';
	import AuthShell from '#lib/auth/AuthShell.svelte';
	import CodeField from '#lib/auth/CodeField.svelte';
	import FormAlert from '#lib/auth/FormAlert.svelte';
	import PasswordField from '#lib/auth/PasswordField.svelte';
	import { t } from '#lib/i18n.svelte.ts';
	import { pop, vanish } from '#lib/motion.ts';
	import { refresh } from '#lib/session.svelte.ts';

	let email = $state('');
	let password = $state('');
	let code = $state('');
	let error = $state({ text: '', n: 0 });
	let busy = $state(false);
	let passwordField: PasswordField | undefined = $state();
	let codeField: CodeField | undefined = $state();

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		passwordField?.hide();
		busy = true;
		const { error: failure, response } = await api.POST('/api/v1/sessions', {
			body: { email, password, code }
		});
		busy = false;
		if (failure || !response.ok) {
			const text =
				response?.status === 401
					? t('login.wrong')
					: problem(failure, response?.status, t('login.failed'));
			error = { text, n: error.n + 1 };
			code = '';
			codeField?.shake();
			return;
		}
		await refresh();
		const next = page.url.searchParams.get('next');
		await goto(next?.startsWith('/') && !next.startsWith('//') ? next : '/');
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
		<CodeField
			bind:this={codeField}
			label={t('login.code')}
			hint={t('login.code_hint')}
			bind:value={code}
		/>
		<div class="actions">
			<FormAlert {...error} />
			<button type="submit" class="primary" disabled={busy}>
				{#key busy}
					<span in:pop={{ start: 0.9 }} out:vanish>{busy ? t('login.busy') : t('login.title')}</span
					>
				{/key}
			</button>
		</div>
	</form>
</AuthShell>
