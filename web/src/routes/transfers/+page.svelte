<script lang="ts">
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import Made from '#lib/transfers/Made.svelte';
	import Summary from '#lib/transfers/Summary.svelte';
	import Unmatched from '#lib/transfers/Unmatched.svelte';
	import Waiting from '#lib/transfers/Waiting.svelte';
	import { accountNames, type AccountNames } from '#lib/transfers/legs.ts';
	import { t } from '#lib/i18n.svelte.ts';

	let preview = $state<Schemas['TransferPreview'] | null>(null);
	let transfers = $state<Schemas['Transaction'][]>([]);
	let names = $state<AccountNames>(new Map());
	let loadError = $state('');
	let version = $state(0);

	async function load() {
		loadError = '';
		const [p, x, a] = await Promise.all([
			api.POST('/api/v1/transfers/preview'),
			api.GET('/api/v1/transactions', { params: { query: { transfers: true, limit: 200 } } }),
			api.GET('/api/v1/accounts')
		]);
		if (!p.data || !x.data || !a.data) {
			const failed = !p.data ? p : !x.data ? x : a;
			loadError = problem(failed.error, failed.response?.status, t('transfers.failed'));
			return;
		}
		preview = p.data;
		transfers = x.data.items;
		names = accountNames(a.data.items);
		version += 1;
	}

	onMount(load);
</script>

<svelte:head>
	<title>{t('app.title', { page: t('transfers.title') })}</title>
</svelte:head>

<h1>{t('transfers.title')}</h1>
<p class="hint">{t('transfers.hint')}</p>

{#if loadError}
	<p class="alert" role="alert">
		{loadError} <button class="secondary" onclick={load}>{t('app.try_again')}</button>
	</p>
{/if}
{#if preview}
	<div class="stack">
		<Summary {version} />
		<Waiting {preview} {names} onchanged={load} />
		<Unmatched {preview} {names} onchanged={load} />
		<Made {transfers} {names} onchanged={load} />
	</div>
{:else if !loadError}
	<p class="hint">{t('app.loading')}</p>
{/if}
