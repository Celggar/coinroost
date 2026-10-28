<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api, problem } from '#lib/api/client.ts';
	import LanguageSwitch from '#lib/LanguageSwitch.svelte';
	import { t } from '#lib/i18n.svelte.ts';
	import { refresh } from '#lib/session.svelte.ts';

	let email = $state('');
	let password = $state('');
	let code = $state('');
	let error = $state('');
	let busy = $state(false);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = '';
		const { error: failure, response } = await api.POST('/api/v1/sessions', {
			body: { email, password, code }
		});
		busy = false;
		if (failure || !response.ok) {
			error =
				response?.status === 401
					? t('login.wrong')
					: problem(failure, response?.status, t('login.failed'));
			code = '';
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

<section class="login card">
	<div class="heading">
		<h1>{t('app.name')}</h1>
		<LanguageSwitch />
	</div>
	<form class="stack" onsubmit={submit}>
		<label>
			{t('login.email')}
			<input type="email" autocomplete="username" required bind:value={email} />
		</label>
		<label>
			{t('login.password')}
			<input type="password" autocomplete="current-password" required bind:value={password} />
		</label>
		<label>
			{t('login.code')}
			<span class="hint">{t('login.code_hint')}</span>
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
		<button type="submit" disabled={busy}>{busy ? t('login.busy') : t('login.title')}</button>
	</form>
</section>

<style>
	.login {
		max-width: 420px;
		margin: 10vh auto 0;
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
</style>
