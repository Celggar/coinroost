<script lang="ts">
	import { retry } from '#lib/ui/focus.ts';
	import { onMount } from 'svelte';
	import { onMoved } from '#lib/nav/counts.svelte.ts';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import Made from '#lib/transfers/Made.svelte';
	import Summary from '#lib/transfers/Summary.svelte';
	import Unmatched from '#lib/transfers/Unmatched.svelte';
	import Waiting from '#lib/transfers/Waiting.svelte';
	import { accountNames, type AccountNames } from '#lib/transfers/legs.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import { arrive, placeholder } from '#lib/motion.ts';
	import FirstStep from '#lib/ui/FirstStep.svelte';

	let preview = $state<Schemas['TransferPreview'] | null>(null);
	let transfers = $state<Schemas['Transaction'][]>([]);
	let names = $state<AccountNames>(new Map());
	let accounts = $state<Schemas['Account'][]>([]);
	// Nothing imported yet: the first step, as on Uncategorized, instead of four cards with nothing in them.
	let empty = $state(false);
	let loadError = $state('');
	let version = $state(0);

	async function load() {
		loadError = '';
		const [p, x, a, any] = await Promise.all([
			api.POST('/api/v1/transfers/preview'),
			api.GET('/api/v1/transactions', { params: { query: { transfers: true, limit: 200 } } }),
			api.GET('/api/v1/accounts'),
			api.GET('/api/v1/transactions', { params: { query: { limit: 1 } } })
		]);
		if (!p.data || !x.data || !a.data || !any.data) {
			const failed = !p.data ? p : !x.data ? x : !a.data ? a : any;
			loadError = problem(failed.error, failed.response?.status, t('transfers.failed'));
			return;
		}
		preview = p.data;
		transfers = x.data.items;
		names = accountNames(a.data.items);
		accounts = a.data.items;
		empty = any.data.items.length === 0;
		version += 1;
	}

	onMount(load);
	onMoved(load);
</script>

<svelte:head>
	<title>{t('app.title', { page: t('transfers.title') })}</title>
</svelte:head>

<h1>{t('transfers.title')}</h1>
<p class="hint">{t('transfers.hint')}</p>

{#if loadError}
	<p class="alert" role="alert" in:arrive out:placeholder>
		{loadError}
		<button class="secondary" onclick={(event) => retry(event.currentTarget, load)}
			>{t('app.try_again')}</button
		>
	</p>
{/if}
{#if preview && empty}
	<div in:arrive><FirstStep {accounts} /></div>
{:else if preview}
	<div class="stack" in:arrive>
		<Summary {version} />
		<Waiting {preview} {names} onchanged={load} />
		<Unmatched {preview} {names} onchanged={load} />
		<Made {transfers} {names} onchanged={load} />
	</div>
{:else if !loadError}
	<p class="hint loading" out:placeholder>{t('app.loading')}</p>
{/if}
