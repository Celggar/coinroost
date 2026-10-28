<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api, problem } from '#lib/api/client.ts';
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
					? 'The email, password or code is not right. Codes change every 30 seconds.'
					: problem(failure, response?.status, 'Logging in failed');
			code = '';
			return;
		}
		await refresh();
		const next = page.url.searchParams.get('next');
		await goto(next?.startsWith('/') && !next.startsWith('//') ? next : '/import');
	}
</script>

<svelte:head>
	<title>Log in · Coinroost</title>
</svelte:head>

<section class="login card">
	<h1>Coinroost</h1>
	<form class="stack" onsubmit={submit}>
		<label>
			Email
			<input type="email" autocomplete="username" required bind:value={email} />
		</label>
		<label>
			Password
			<input type="password" autocomplete="current-password" required bind:value={password} />
		</label>
		<label>
			Code
			<span class="hint">The 6 digits your authenticator app shows now.</span>
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
		<button type="submit" disabled={busy}>{busy ? 'Logging in…' : 'Log in'}</button>
	</form>
</section>

<style>
	.login {
		max-width: 420px;
		margin: 10vh auto 0;
	}
	h1 {
		margin: 0;
	}
</style>
