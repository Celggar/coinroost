<script lang="ts">
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { t, type Key } from '#lib/i18n.svelte.ts';

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
	let created = $state('');
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
		busy = true;
		formError = '';
		created = '';
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
			formError = problem(error, response?.status, t('accounts.create_failed'));
			return;
		}
		created = data.name;
		accounts = [...accounts, data].sort((a, b) => a.name.localeCompare(b.name));
		name = '';
		profile = '';
		namespace = '';
		namespaceEdited = false;
		markers = '';
		cardPrefixes = '';
	}

	const profileName = (id: string | null | undefined) =>
		profiles.find((p) => p.id === id)?.name ?? (id ? id : '—');

	onMount(load);
</script>

<svelte:head>
	<title>{t('app.title', { page: t('accounts.title') })}</title>
</svelte:head>

<h1>{t('accounts.title')}</h1>

{#if loadError}
	<p class="alert" role="alert">
		{loadError} <button class="secondary" onclick={load}>{t('app.try_again')}</button>
	</p>
{:else if loaded}
	<div class="stack">
		{#if accounts.length}
			<div class="table-wrap">
				<table>
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
							<tr>
								<td>{account.name}</td>
								<td>{kindName(account.kind)}</td>
								<td>{account.currency}</td>
								<td>{profileName(account.profile)}</td>
								<td><code>{account.namespace}</code></td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{:else}
			<p>{t('accounts.none_yet')}</p>
		{/if}

		<form class="card" onsubmit={submit}>
			<h2>{t('accounts.new')}</h2>
			<div class="fields">
				<label>
					{t('accounts.name')}
					<input required maxlength="200" bind:value={name} />
				</label>
				<label>
					{t('accounts.kind')}
					<select bind:value={kind}>
						{#each kinds as option (option)}
							<option value={option}>{kindName(option)}</option>
						{/each}
					</select>
				</label>
				<label>
					{t('accounts.bank_file')}
					<span class="hint">{t('accounts.bank_file_hint')}</span>
					<select bind:value={profile} onchange={pickProfile}>
						<option value="">{t('accounts.no_profile')}</option>
						{#each profiles as option (option.id)}
							<option value={option.id}>{option.name} ({option.format.toUpperCase()})</option>
						{/each}
					</select>
				</label>
				<label>
					{t('accounts.currency')}
					<input required maxlength="3" bind:value={currency} />
				</label>
				<label>
					{t('accounts.key')}
					<span class="hint">{t('accounts.key_hint')}</span>
					<input
						required
						maxlength="64"
						value={namespaceEdited ? namespace : suggested}
						oninput={(event) => {
							namespace = event.currentTarget.value;
							namespaceEdited = true;
						}}
					/>
				</label>
				<label>
					{t('accounts.markers')}
					<span class="hint">{t('accounts.markers_hint')}</span>
					<input bind:value={markers} />
				</label>
				<label>
					{t('accounts.prefixes')}
					<span class="hint">{t('accounts.prefixes_hint')}</span>
					<input inputmode="numeric" bind:value={cardPrefixes} />
				</label>
			</div>
			{#if formError}<p class="alert" role="alert">{formError}</p>{/if}
			{#if created}<p class="notice" role="status">
					{t('accounts.created', { name: created })}
				</p>{/if}
			<div><button type="submit" disabled={busy}>{t('accounts.create')}</button></div>
		</form>
	</div>
{:else}
	<p class="hint">{t('app.loading')}</p>
{/if}

<style>
	.fields {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
		gap: 12px;
	}
	code {
		font-size: 0.9rem;
	}
</style>
