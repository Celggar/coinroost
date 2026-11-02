<script lang="ts">
	// The person's recovery codes: how many are left and when they were made, and a new set on demand. Making one asks
	// for the code the authenticator shows now (the server counts wrong ones toward the login lockout), shows the new
	// codes once and retires the old ones.
	import { retry } from '#lib/ui/focus.ts';
	import { onMount, tick } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import CodeField from '#lib/auth/CodeField.svelte';
	import FormAlert from '#lib/auth/FormAlert.svelte';
	import RecoveryCodes from '#lib/auth/RecoveryCodes.svelte';
	import { plural, tag, t } from '#lib/i18n.svelte.ts';
	import { arrive, followContent, placeholder, pop, rise, swapOut, vanish } from '#lib/motion.ts';
	import { session } from '#lib/session.svelte.ts';
	import { check, focusFirst, showWhole } from '#lib/ui/form.ts';

	const LOW = 3;

	let status = $state<Schemas['RecoveryStatus'] | null>(null);
	let loadError = $state('');
	let mode = $state<'view' | 'confirm' | 'show'>('view');
	let fresh = $state<Schemas['RecoveryCodes'] | null>(null);
	let code = $state('');
	let error = $state({ text: '', n: 0 });
	// What is wrong with the code itself goes under the slots (FD5); the alert keeps what is not about the field.
	let codeError = $state('');
	let busy = $state(false);
	let codeField: CodeField | undefined = $state();
	let regenerateButton: HTMLButtonElement | undefined = $state();
	let freshTitle: HTMLHeadingElement | undefined = $state();

	/** The view that came in takes the focus: the button that was pressed left with the old one. Focused without a
	 * scroll, and the page glides to it: a bare focus scrolls in one frame. `held`: the card, while its room is held. */
	async function show(next: typeof mode, held?: HTMLElement) {
		mode = next;
		await tick();
		if (next === 'confirm') codeField?.focus({ preventScroll: true });
		else if (next === 'show') freshTitle?.focus({ preventScroll: true });
		else regenerateButton?.focus({ preventScroll: true });
		const target = document.activeElement;
		if (target instanceof HTMLElement && target !== document.body)
			await showWhole(target, target, held);
	}

	// Its spaces do not break: «31 oct» never ends a line with «2026» alone under it.
	const made = (iso: string) =>
		new Intl.DateTimeFormat(tag(), { day: 'numeric', month: 'short', year: 'numeric' })
			.format(new Date(iso))
			.replaceAll(' ', '\u00a0');

	async function load() {
		loadError = '';
		const { data, error: failure, response } = await api.GET('/api/v1/me/recovery-codes');
		if (data) status = data;
		else loadError = problem(failure, response.status, t('security.load_failed'));
	}

	function ask() {
		code = '';
		error = { text: '', n: error.n + 1 };
		show('confirm');
	}

	async function regenerate(event: SubmitEvent) {
		event.preventDefault();
		if (busy) return;
		if (refused(event)) return;
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
			await load();
			await show('show');
			return;
		}
		code = '';
		codeField?.shake();
		// A wrong code is about the field: its error goes under it, and the caret goes back to it for the next try.
		if (response.status === 422) {
			codeError = t('setup.wrong_code');
			codeField?.focus();
			return;
		}
		const text =
			response.status === 403
				? t('security.locked')
				: problem(failure, response.status, t('security.failed'));
		error = { text, n: error.n + 1 };
	}

	let card = $state<HTMLElement>();
	// Held while the codes or the form leave: the page glides into the card's new end first, and the card gives back
	// their room after. Let go at once at the page's end, the page was pulled back 165 px in one frame (pass 16 sweep).
	let holding = $state(false);

	async function done() {
		holding = true;
		fresh = null;
		await show('view', card);
		holding = false;
	}

	onMount(load);

	/** Stops a send the browser's checks refuse: one message in the page's language, the field marked and focused. */
	function refused(event: SubmitEvent): boolean {
		const form = event.currentTarget as HTMLFormElement;
		const errors = check(form);
		const why = Object.values(errors)[0];
		if (!why) return false;
		codeError = why;
		focusFirst(form, errors);
		return true;
	}
</script>

<svelte:head>
	<title>{t('app.title', { page: t('security.title') })}</title>
</svelte:head>

<h1>{t('security.title')}</h1>

<!-- The card shrinks as the new codes leave instead of at once: at the page's end «Listo, los guardé» pulled the page
     back 165 px in one frame (pass 16 sweep). -->
<section
	class="card codes follows"
	aria-labelledby="codes-title"
	bind:this={card}
	use:followContent={holding}
>
	<div class="stack">
		<h2 id="codes-title">{t('security.codes_title')}</h2>
		<p class="hint">{t('security.codes_about')}</p>
		{#if loadError}
			<p class="alert" role="alert" in:arrive out:placeholder>
				{loadError}
				<button
					class="secondary"
					onclick={(event) => retry(event.currentTarget, load, '#codes-title')}
					>{t('app.try_again')}</button
				>
			</p>
		{:else if status}
			<p class="count" data-testid="recovery-status" in:arrive>
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
				<div class="mode" in:rise={{ delay: 100 }} out:swapOut>
					{#if mode === 'view'}
						<!-- With no codes yet there is nothing to replace: the button makes the first ones. Only once the status is
						     known: before it, or when it failed, «Crear códigos» would replace codes the person may have. -->
						{#if status}
							<button
								type="button"
								class="primary"
								onclick={ask}
								bind:this={regenerateButton}
								in:rise
								>{status.created_at ? t('security.regenerate') : t('security.confirm')}</button
							>
						{/if}
					{:else if mode === 'confirm'}
						<form novalidate oninput={() => (codeError = '')} class="stack" onsubmit={regenerate}>
							{#if status?.created_at}<p class="hint">{t('security.regenerate_note')}</p>{/if}
							<CodeField
								bind:this={codeField}
								label={t('login.code')}
								hint={t('login.code_hint')}
								hintBelow
								error={codeError}
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
								<button type="button" class="secondary" onclick={done}
									>{t('security.cancel')}</button
								>
							</div>
						</form>
					{:else if fresh}
						<div class="stack">
							<div>
								<h3 tabindex="-1" bind:this={freshTitle}>{t('security.new_title')}</h3>
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
	</div>
</section>

<style>
	/* Clipped at the padding's edge, the buttons' focus rings stay whole. */
	.follows {
		position: relative;
		overflow: clip;
		align-content: start;
	}
	@media (prefers-reduced-motion: no-preference) {
		.follows {
			transition: height var(--duration-medium) var(--ease-standard);
		}
	}
	.codes {
		max-width: 560px;
	}
	/* On the phone layout the column sits on the bottom bar's axis, not left of it (pass 7, at 899 px). */
	@media (max-width: 899px) {
		.codes {
			margin-inline: auto;
		}
	}
	h2,
	h3 {
		margin: 0;
	}
	h3 {
		font-size: var(--text-lg);
	}
	/* Focused by the page when the new codes arrive, so a screen reader starts there; it is not a control. */
	h3:focus {
		outline: none;
	}
	p {
		margin: 0;
	}
	.count {
		font-weight: 600;
	}
	.warn {
		border-radius: var(--radius);
		padding: var(--space-md) var(--space-lg);
		background: var(--warn-bg);
		color: var(--warn);
	}
	.cell {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
	}
	/* Nothing to do yet (the status did not load): no empty row adding a gap under the alert. */
	.cell:not(:has(.mode > *)) {
		display: none;
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
		gap: var(--space-sm);
	}
	/* A row with the primary in it: every button takes the primary's height (FD6), so the row shares its edges. */
	.buttons button {
		display: grid;
		place-items: center;
		min-height: var(--primary-height);
	}
	.buttons button > :global(*) {
		grid-area: 1 / 1;
	}
</style>
