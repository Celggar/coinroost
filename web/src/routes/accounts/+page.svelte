<script lang="ts">
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';

	let accounts = $state<Schemas['Account'][]>([]);
	let profiles = $state<Schemas['BankProfile'][]>([]);
	let loadError = $state('');
	let loaded = $state(false);

	const kinds: { value: Schemas['AccountKind']; label: string }[] = [
		{ value: 'checking', label: 'Checking' },
		{ value: 'savings', label: 'Savings' },
		{ value: 'credit_card', label: 'Credit card' },
		{ value: 'cash', label: 'Cash' },
		{ value: 'loan', label: 'Loan' },
		{ value: 'investment', label: 'Investment' }
	];

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
			loadError = problem(
				failed.error,
				failed.response?.status,
				'The accounts could not be loaded'
			);
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
			formError = problem(error, response?.status, 'The account could not be created');
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
	<title>Accounts · Coinroost</title>
</svelte:head>

<h1>Accounts</h1>

{#if loadError}
	<p class="alert" role="alert">
		{loadError} <button class="secondary" onclick={load}>Try again</button>
	</p>
{:else if loaded}
	<div class="stack">
		{#if accounts.length}
			<div class="table-wrap">
				<table>
					<thead>
						<tr>
							<th>Name</th>
							<th>Kind</th>
							<th>Currency</th>
							<th>Bank file</th>
							<th>Key</th>
						</tr>
					</thead>
					<tbody>
						{#each accounts as account (account.id)}
							<tr>
								<td>{account.name}</td>
								<td>{kinds.find((k) => k.value === account.kind)?.label}</td>
								<td>{account.currency}</td>
								<td>{profileName(account.profile)}</td>
								<td><code>{account.namespace}</code></td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{:else}
			<p>No accounts yet.</p>
		{/if}

		<form class="card" onsubmit={submit}>
			<h2>New account</h2>
			<div class="fields">
				<label>
					Name
					<input required maxlength="200" bind:value={name} />
				</label>
				<label>
					Kind
					<select bind:value={kind}>
						{#each kinds as option (option.value)}
							<option value={option.value}>{option.label}</option>
						{/each}
					</select>
				</label>
				<label>
					Bank file
					<span class="hint">How its statements are read; none for cash.</span>
					<select bind:value={profile} onchange={pickProfile}>
						<option value="">None</option>
						{#each profiles as option (option.id)}
							<option value={option.id}>{option.name} ({option.format.toUpperCase()})</option>
						{/each}
					</select>
				</label>
				<label>
					Currency
					<input required maxlength="3" bind:value={currency} />
				</label>
				<label>
					Key
					<span class="hint"
						>Goes into every imported transaction's id; it cannot change later.</span
					>
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
					Header texts
					<span class="hint">Optional, comma separated: texts its files always contain.</span>
					<input bind:value={markers} />
				</label>
				<label>
					Card prefixes
					<span class="hint"
						>Optional, for cards that share a format: 4 for Visa, 34, 37 for Amex.</span
					>
					<input inputmode="numeric" bind:value={cardPrefixes} />
				</label>
			</div>
			{#if formError}<p class="alert" role="alert">{formError}</p>{/if}
			{#if created}<p class="notice" role="status">Created {created}.</p>{/if}
			<div><button type="submit" disabled={busy}>Create account</button></div>
		</form>
	</div>
{:else}
	<p class="hint">Loading…</p>
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
