<script lang="ts">
	// The person's recovery codes: how many are left and when they were made, and a new set on demand. Making one asks
	// for the code the authenticator shows now (the server counts wrong ones toward the login lockout), shows the new
	// codes once and retires the old ones.
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import CodeField from '#lib/auth/CodeField.svelte';
	import FormAlert from '#lib/auth/FormAlert.svelte';
	import RecoveryCodes from '#lib/auth/RecoveryCodes.svelte';
	import { plural, tag, t } from '#lib/i18n.svelte.ts';
	import { pop, rise, swapOut, vanish } from '#lib/motion.ts';
	import { session } from '#lib/session.svelte.ts';

	const LOW = 3;

	let status = $state<Schemas['RecoveryStatus'] | null>(null);
	let loadError = $state('');
	let mode = $state<'view' | 'confirm' | 'show'>('view');
	let fresh = $state<Schemas['RecoveryCodes'] | null>(null);
	let code = $state('');
	let error = $state({ text: '', n: 0 });
	let busy = $state(false);
	let codeField: CodeField | undefined = $state();

	const made = (iso: string) =>
		new Intl.DateTimeFormat(tag(), { day: 'numeric', month: 'short', year: 'numeric' }).format(
			new Date(iso)
		);

	async function load() {
		loadError = '';
		const { data, error: failure, response } = await api.GET('/api/v1/me/recovery-codes');
		if (data) status = data;
		else loadError = problem(failure, response.status, t('security.load_failed'));
	}

	function ask() {
		code = '';
		error = { text: '', n: error.n + 1 };
		mode = 'confirm';
	}

	async function regenerate(event: SubmitEvent) {
		event.preventDefault();
		if (busy) return;
		busy = true;
		const {
			data,
			error: failure,
			response
		} = await api.POST('/api/v1/me/recovery-codes', {
			body: { code }
		});
		busy = false;
		if (data) {
			fresh = data;
			mode = 'show';
			await load();
			return;
		}
		code = '';
		const text =
			response.status === 422
				? t('setup.wrong_code')
				: response.status === 403
					? t('security.locked')
					: problem(failure, response.status, t('security.failed'));
		error = { text, n: error.n + 1 };
		codeField?.shake();
	}

	function done() {
		fresh = null;
		mode = 'view';
	}

	onMount(load);
</script>

<svelte:head>
	<title>{t('app.title', { page: t('security.title') })}</title>
</svelte:head>

<h1>{t('security.title')}</h1>

<section class="card codes" aria-labelledby="codes-title">
	<h2 id="codes-title">{t('security.codes_title')}</h2>
	<p class="hint">{t('security.codes_about')}</p>
	{#if loadError}
		<p class="alert" role="alert">
			{loadError} <button class="secondary" onclick={load}>{t('app.try_again')}</button>
		</p>
	{:else if status}
		<p class="count" data-testid="recovery-status" in:rise>
			{#if status.created_at}
				{plural(status.remaining, 'security.codes_left_one', 'security.codes_left_other', {
					size: status.size,
					date: made(status.created_at)
				})}
			{:else}
				{t('security.codes_none')}
			{/if}
		</p>
		{#if status.created_at && status.remaining <= LOW && mode !== 'show'}
			<p class="warn" in:rise out:vanish>{t('security.codes_low')}</p>
		{/if}
	{/if}

	<div class="cell">
		{#key mode}
			<div class="mode" in:rise={{ delay: 150 }} out:swapOut>
				{#if mode === 'view'}
					<button type="button" class="primary" onclick={ask}>{t('security.regenerate')}</button>
				{:else if mode === 'confirm'}
					<form class="stack" onsubmit={regenerate}>
						<p class="hint">{t('security.regenerate_note')}</p>
						<CodeField
							bind:this={codeField}
							label={t('login.code')}
							hint={t('login.code_hint')}
							bind:value={code}
						/>
						<FormAlert {...error} />
						<div class="buttons">
							<button type="submit" class="primary" aria-disabled={busy}>
								{#key busy}
									<span in:pop={{ start: 0.9 }} out:vanish>
										{busy ? t('security.busy') : t('security.confirm')}
									</span>
								{/key}
							</button>
							<button type="button" class="secondary" onclick={done}>{t('security.cancel')}</button>
						</div>
					</form>
				{:else if fresh}
					<div class="stack">
						<div>
							<h3>{t('security.new_title')}</h3>
							<p class="hint">{t('security.new_lead')}</p>
						</div>
						<RecoveryCodes
							codes={fresh.codes}
							email={session.user?.email ?? ''}
							createdAt={fresh.created_at}
						/>
						<div class="buttons">
							<button type="button" class="primary" onclick={done}>{t('security.done')}</button>
						</div>
					</div>
				{/if}
			</div>
		{/key}
	</div>
</section>

<style>
	.codes {
		max-width: 560px;
	}
	h2,
	h3 {
		margin: 0;
	}
	h3 {
		font-size: 1.05rem;
	}
	p {
		margin: 0;
	}
	.count {
		font-weight: 600;
	}
	.warn {
		border-radius: var(--radius);
		padding: 10px 12px;
		background: var(--warn-bg);
		color: var(--warn);
	}
	.cell {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
	}
	.mode {
		grid-area: 1 / 1;
		display: grid;
		min-width: 0;
	}
	.mode > button {
		justify-self: start;
	}
	.buttons {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}
	.buttons button {
		display: grid;
		place-items: center;
		min-height: 44px;
	}
	.buttons button > :global(*) {
		grid-area: 1 / 1;
	}
</style>
