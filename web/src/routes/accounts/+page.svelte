<script lang="ts">
	import { focusMade, focusOn, retry } from '#lib/ui/focus.ts';
	import { scrollable } from '#lib/ui/scrollable.ts';
	import { onMount, tick } from 'svelte';
	import AccountForm from '#lib/accounts/AccountForm.svelte';
	import { formatName, kindName } from '#lib/accounts/names.ts';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import { arrive, placeholder, rise, vanish } from '#lib/motion.ts';
	import { follow, openRoom } from '#lib/ui/hold-room.ts';
	import { showWhole } from '#lib/ui/landing.ts';

	let accounts = $state<Schemas['Account'][]>([]);
	let profiles = $state<Schemas['BankProfile'][]>([]);
	let loadError = $state('');
	let loaded = $state(false);

	let adding = $state(false);

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

	/** Opens the form on the name field, the page moving as the form grows. */
	async function add() {
		adding = true;
		await tick();
		focusOn('form input[name="name"]');
	}

	async function close() {
		adding = false;
		await tick();
		focusOn('#new-account');
	}

	/** The form closes and the new account, in its place in the list, takes the focus and shows whole. */
	async function created(account: Schemas['Account']) {
		accounts = [...accounts, account].sort((a, b) => a.name.localeCompare(b.name));
		adding = false;
		await tick();
		const row = focusMade(`[data-account="${account.id}"]`);
		if (row) await showWhole(row);
	}

	const profileName = (id: string | null | undefined) => {
		const profile = profiles.find((p) => p.id === id);
		return profile ? formatName(profile) : id ? id : '—';
	};

	onMount(load);
</script>

<svelte:head>
	<title>{t('app.title', { page: t('accounts.title') })}</title>
</svelte:head>

<div class="title-bar">
	<h1>{t('accounts.title')}</h1>
	{#if loaded && accounts.length && !adding}
		<button id="new-account" onclick={add} in:vanish out:vanish>+ {t('accounts.add')}</button>
	{/if}
</div>

{#if loadError}
	<p class="alert" role="alert" in:arrive out:placeholder>
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
							<tr data-account={account.id} in:rise>
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

		{#if adding || accounts.length === 0}
			<div transition:openRoom onintrostart={follow} onoutroend={follow}>
				<AccountForm
					{profiles}
					oncreated={created}
					oncancel={accounts.length ? close : undefined}
				/>
			</div>
		{/if}
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
