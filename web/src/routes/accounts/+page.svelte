<script lang="ts">
	import { retry } from '#lib/ui/focus.ts';
	import { scrollable } from '#lib/ui/scrollable.ts';
	import { onMount, tick } from 'svelte';
	import { api, failure, problem, type Schemas } from '#lib/api/client.ts';
	import { has, t, type Key } from '#lib/i18n.svelte.ts';
	import { arrive, placeholder, rise, vanish } from '#lib/motion.ts';
	import Field from '#lib/ui/Field.svelte';
	import FormError from '#lib/ui/FormError.svelte';
	import { check, drop, focusFirst, without, type Errors } from '#lib/ui/form.ts';
	import { notify } from '#lib/ui/notices.svelte.ts';

	let accounts = $state<Schemas['Account'][]>([]);
	let profiles = $state<Schemas['BankProfile'][]>([]);
	let loadError = $state('');
	let loaded = $state(false);

	// A new kind in the contract fails the type check until it has a name here.
	const kindKeys = {
		checking: 'kind.checking',
		savings: 'kind.savings',
		credit_card: 'kind.credit_card',
		cash: 'kind.cash',
		loan: 'kind.loan',
		investment: 'kind.investment'
	} satisfies Record<Schemas['AccountKind'], Key>;
	const kinds = Object.keys(kindKeys) as Schemas['AccountKind'][];
	const kindName = (kind: Schemas['AccountKind']) => t(kindKeys[kind]);

	let name = $state('');
	let kind = $state<Schemas['AccountKind']>('checking');
	let currency = $state('USD');
	let profile = $state('');
	let namespace = $state('');
	let namespaceEdited = $state(false);
	let markers = $state('');
	let cardPrefixes = $state('');
	let formError = $state('');
	let errors = $state<Errors>({});
	let busy = $state(false);

	const suggested = $derived(
		name
			.normalize('NFD')
			.replace(/[̀-ͯ]/g, '')
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, '-')
			.replace(/^-+|-+$/g, '')
			.slice(0, 64)
	);

	async function load() {
		loadError = '';
		const [list, formats] = await Promise.all([
			api.GET('/api/v1/accounts'),
			api.GET('/api/v1/profiles')
		]);
		if (!list.data || !formats.data) {
			const failed = list.data ? formats : list;
			loadError = problem(failed.error, failed.response?.status, t('import.accounts_failed'));
			return;
		}
		accounts = list.data.items;
		profiles = formats.data.items;
		loaded = true;
	}

	function pickProfile() {
		const chosen = profiles.find((p) => p.id === profile);
		if (chosen) currency = chosen.currency;
	}

	const words = (text: string) =>
		text
			.split(',')
			.map((w) => w.trim())
			.filter(Boolean);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		const form = event.currentTarget as HTMLFormElement;
		formError = '';
		errors = check(form);
		// A key that still follows the name is empty only because the name is: the name's error says it (pass 8).
		if (!namespaceEdited && errors.name) errors = drop(errors, 'namespace');
		if (Object.keys(errors).length) {
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
				await tick();
				focusFirst(form, errors);
			} else {
				formError = why.message;
			}
			return;
		}
		notify(t('accounts.created', { name: data.name }));
		accounts = [...accounts, data].sort((a, b) => a.name.localeCompare(b.name));
		name = '';
		profile = '';
		namespace = '';
		namespaceEdited = false;
		markers = '';
		cardPrefixes = '';
		// The button was disabled while it worked, which drops the focus: it goes back to the form, ready for the next.
		(form.elements.namedItem('name') as HTMLInputElement | null)?.focus();
	}

	/** A bank format's name in the page's language; the profile's own (English) name when there is no text for it. */
	const formatName = (profile: { id: string; name: string }) => {
		const key = `profile.${profile.id}`;
		return has(key) ? t(key) : profile.name;
	};
	const profileName = (id: string | null | undefined) => {
		const profile = profiles.find((p) => p.id === id);
		return profile ? formatName(profile) : id ? id : '—';
	};

	onMount(load);
</script>

<svelte:head>
	<title>{t('app.title', { page: t('accounts.title') })}</title>
</svelte:head>

<h1>{t('accounts.title')}</h1>

{#if loadError}
	<p class="alert" role="alert" out:placeholder>
		{loadError}
		<button class="secondary" onclick={(event) => retry(event.currentTarget, load)}
			>{t('app.try_again')}</button
		>
	</p>
{:else if loaded}
	<div class="stack" in:arrive>
		{#if accounts.length}
			<div class="table-wrap" in:rise use:scrollable={t('accounts.title')}>
				<table class="rows-on-phone">
					<thead>
						<tr>
							<th>{t('accounts.name')}</th>
							<th>{t('accounts.kind')}</th>
							<th>{t('accounts.currency')}</th>
							<th>{t('accounts.bank_file')}</th>
							<th>{t('accounts.key')}</th>
						</tr>
					</thead>
					<tbody>
						{#each accounts as account (account.id)}
							<tr in:rise>
								<td data-label={t('accounts.name')}>{account.name}</td>
								<td data-label={t('accounts.kind')} class="kind">{kindName(account.kind)}</td>
								<td data-label={t('accounts.currency')}>{account.currency}</td>
								<td class="bank-file" data-label={t('accounts.bank_file')}
									>{profileName(account.profile)}</td
								>
								<td data-label={t('accounts.key')}><code>{account.namespace}</code></td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{:else}
			<p out:vanish>{t('accounts.none_yet')}</p>
		{/if}

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
			<div class="fields">
				<Field label={t('accounts.name')} name="name" error={errors.name}>
					{#snippet children(wiring)}
						<input {...wiring} required maxlength="200" bind:value={name} />
					{/snippet}
				</Field>
				<Field label={t('accounts.kind')} name="kind" error={errors.kind}>
					{#snippet children(wiring)}
						<select {...wiring} bind:value={kind}>
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
						<input {...wiring} required maxlength="3" bind:value={currency} />
					{/snippet}
				</Field>
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
			<FormError text={formError} />
			<div class="actions">
				<button type="submit" disabled={busy}>{t('accounts.create')}</button>
			</div>
		</form>
	</div>
{:else}
	<p class="hint loading" out:placeholder>{t('app.loading')}</p>
{/if}

<style>
	/* Where the table has the room, a bank file's name keeps to one line and the key, which breaks at its hyphens,
	   takes the rest: «BAC · credit / card (CSV)» broke beside a 300 px key (pass 9). Narrower, it wraps, or the
	   name column would shrink to a word per line (tried at 600 px). */
	@media (min-width: 1100px) {
		.bank-file {
			white-space: nowrap;
		}
	}
	code {
		font-size: var(--text-sm);
	}
	/* A kind is a short name («Tarjeta de crédito»): it holds on one line instead of leaving a word alone. */
	.kind {
		white-space: nowrap;
	}
</style>
