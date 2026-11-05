<script lang="ts">
	// A new account: the four fields a person knows, and what only some accounts need folded under «Opciones avanzadas»,
	// the key among them with its warning that it cannot change. `oncancel` is absent while there is no account: the
	// form is then the page's first step and stays open.
	import { tick } from 'svelte';
	import { api, failure, type Schemas } from '#lib/api/client.ts';
	import { formatName, kindName, kinds } from '#lib/accounts/names.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import Field from '#lib/ui/Field.svelte';
	import FormError from '#lib/ui/FormError.svelte';
	import { check, drop, focusFirst, without, type Errors } from '#lib/ui/form.ts';
	import { follow } from '#lib/ui/hold-room.ts';
	import { notify } from '#lib/ui/notices.svelte.ts';

	let {
		profiles,
		oncreated,
		oncancel
	}: {
		profiles: Schemas['BankProfile'][];
		oncreated: (account: Schemas['Account']) => void;
		oncancel?: () => void;
	} = $props();

	let name = $state('');
	let kind = $state<Schemas['AccountKind']>('checking');
	// The kind the person chose themselves: a bank file that reads one kind sets it, and another file gives it back.
	let kindByHand: Schemas['AccountKind'] = 'checking';
	let currency = $state('USD');
	let profile = $state('');
	let namespace = $state('');
	let namespaceEdited = $state(false);
	let markers = $state('');
	let cardPrefixes = $state('');
	let formError = $state('');
	let errors = $state<Errors>({});
	let busy = $state(false);
	let advanced = $state(false);
	// What the fold holds: an error there opens it, or the field it names could not take the focus.
	const ADVANCED = ['namespace', 'detection.markers', 'detection.card_prefixes'];

	const suggested = $derived(
		name
			.normalize('NFD')
			.replace(/[̀-ͯ]/g, '')
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, '-')
			.replace(/^-+|-+$/g, '')
			.slice(0, 64)
	);
	const key = $derived(namespaceEdited ? namespace : suggested);

	function pickProfile() {
		const chosen = profiles.find((p) => p.id === profile);
		if (chosen) currency = chosen.currency;
		// A card's statement belongs to a credit card: the kind follows the file (pass 20, it stayed «Corriente»).
		kind = chosen?.kind ?? kindByHand;
	}

	const words = (text: string) =>
		text
			.split(',')
			.map((w) => w.trim())
			.filter(Boolean);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (busy) return;
		const form = event.currentTarget as HTMLFormElement;
		formError = '';
		errors = check(form);
		// A key that still follows the name is empty only because the name is: the name's error says it (pass 8).
		if (!namespaceEdited && errors.name) errors = drop(errors, 'namespace');
		if (Object.keys(errors).length) {
			if (ADVANCED.some((field) => errors[field])) advanced = true;
			await tick();
			focusFirst(form, errors);
			return;
		}
		busy = true;
		const { data, error, response } = await api.POST('/api/v1/accounts', {
			body: {
				name,
				kind,
				currency,
				namespace: namespaceEdited ? namespace : suggested,
				profile: profile || null,
				detection: { markers: words(markers), card_prefixes: words(cardPrefixes) }
			}
		});
		busy = false;
		if (!data) {
			const why = failure(error, response?.status, t('accounts.create_failed'));
			if (why.field && form.elements.namedItem(why.field)) {
				errors = { [why.field]: why.message };
				if (ADVANCED.includes(why.field)) advanced = true;
				await tick();
				focusFirst(form, errors);
			} else {
				formError = why.message;
			}
			return;
		}
		notify(t('accounts.created', { name: data.name }));
		oncreated(data);
	}
</script>

<form
	class="card"
	novalidate
	onsubmit={submit}
	oninput={(event) => {
		errors = without(errors, event);
		// The key follows the name until it is edited: a name that fills it also clears its error.
		if (!namespaceEdited && suggested) errors = drop(errors, 'namespace');
	}}
>
	<h2>{t('accounts.new')}</h2>
	<div class="fields four">
		<Field label={t('accounts.name')} name="name" error={errors.name}>
			{#snippet children(wiring)}
				<input {...wiring} required maxlength="200" bind:value={name} />
			{/snippet}
		</Field>
		<Field label={t('accounts.kind')} name="kind" error={errors.kind}>
			{#snippet children(wiring)}
				<select {...wiring} bind:value={kind} onchange={() => (kindByHand = kind)}>
					{#each kinds as option (option)}
						<option value={option}>{kindName(option)}</option>
					{/each}
				</select>
			{/snippet}
		</Field>
		<Field
			label={t('accounts.bank_file')}
			name="profile"
			hint={t('accounts.bank_file_hint')}
			error={errors.profile}
		>
			{#snippet children(wiring)}
				<select {...wiring} bind:value={profile} onchange={pickProfile}>
					<option value="">{t('accounts.no_profile')}</option>
					{#each profiles as option (option.id)}
						<option value={option.id}>{formatName(option)}</option>
					{/each}
				</select>
			{/snippet}
		</Field>
		<Field label={t('accounts.currency')} name="currency" error={errors.currency}>
			{#snippet children(wiring)}
				<!-- The API's rules (validate.rs), checked before sending: one error per send took three sends for
				     three (pass 14). No length cap: «dolares» was cut to «dol» without a word; the pattern says
				     what is wrong instead. -->
				<input
					{...wiring}
					required
					pattern={'[A-Za-z]{3}'}
					data-format="problem.currency_unknown"
					bind:value={currency}
				/>
			{/snippet}
		</Field>
	</div>
	<details bind:open={advanced} ontoggle={follow}>
		<summary>
			{t('accounts.advanced')} · {t('accounts.key').toLowerCase()}
			<code>{key || '…'}</code>
		</summary>
		<div class="fields advanced">
			<Field
				label={t('accounts.key')}
				name="namespace"
				hint={t('accounts.key_hint')}
				error={errors.namespace}
			>
				{#snippet children(wiring)}
					<input
						{...wiring}
						required
						maxlength="64"
						pattern={'[a-z0-9][a-z0-9_\\-]{0,63}'}
						data-format="problem.namespace_format"
						value={namespaceEdited ? namespace : suggested}
						oninput={(event) => {
							namespace = event.currentTarget.value;
							namespaceEdited = true;
						}}
					/>
				{/snippet}
			</Field>
			<Field
				label={t('accounts.markers')}
				name="detection.markers"
				hint={t('accounts.markers_hint')}
				error={errors['detection.markers']}
			>
				{#snippet children(wiring)}
					<input {...wiring} bind:value={markers} />
				{/snippet}
			</Field>
			<Field
				label={t('accounts.prefixes')}
				name="detection.card_prefixes"
				hint={t('accounts.prefixes_hint')}
				error={errors['detection.card_prefixes']}
			>
				{#snippet children(wiring)}
					<input {...wiring} inputmode="numeric" bind:value={cardPrefixes} />
				{/snippet}
			</Field>
		</div>
	</details>
	<FormError text={formError} />
	<div class="actions">
		<!-- aria-disabled, not disabled: a disabled button drops the focus to the page while it works, and it sat there
		     until the page landed (pass 17). -->
		<button type="submit" aria-disabled={busy}>{t('accounts.create')}</button>
		{#if oncancel}
			<button type="button" class="secondary" onclick={oncancel}>{t('accounts.cancel')}</button>
		{/if}
	</div>
</form>

<style>
	/* The four fields in one aligned row where they fit; each starts at the top, whatever an error adds under it. */
	.fields {
		align-items: start;
	}
	@media (min-width: 1100px) {
		.four {
			grid-template-columns: repeat(4, minmax(0, 1fr));
		}
	}
	.advanced {
		padding-top: var(--space-md);
	}
	/* Side by side, the two buttons stand as tall as each other (FD6). */
	.actions {
		align-items: stretch;
	}
	code {
		font-size: var(--text-sm);
	}
</style>
