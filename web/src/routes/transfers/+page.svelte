<script lang="ts">
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import Made from '#lib/transfers/Made.svelte';
	import Summary from '#lib/transfers/Summary.svelte';
	import Unmatched from '#lib/transfers/Unmatched.svelte';
	import Waiting from '#lib/transfers/Waiting.svelte';
	import { accountNames, type AccountNames } from '#lib/transfers/legs.ts';

	let preview = $state<Schemas['TransferPreview'] | null>(null);
	let transfers = $state<Schemas['Transaction'][]>([]);
	let names = $state<AccountNames>(new Map());
	let loadError = $state('');
	let version = $state(0);

	async function load() {
		loadError = '';
		const [p, t, a] = await Promise.all([
			api.POST('/api/v1/transfers/preview'),
			api.GET('/api/v1/transactions', { params: { query: { transfers: true, limit: 200 } } }),
			api.GET('/api/v1/accounts')
		]);
		if (!p.data || !t.data || !a.data) {
			const failed = !p.data ? p : !t.data ? t : a;
			loadError = problem(
				failed.error,
				failed.response?.status,
				'The transfers could not be loaded'
			);
			return;
		}
		preview = p.data;
		transfers = t.data.items;
		names = accountNames(a.data.items);
		version += 1;
	}

	onMount(load);
</script>

<svelte:head>
	<title>Transfers · Coinroost</title>
</svelte:head>

<h1>Transfers</h1>
<p class="hint">
	Money moved between your own accounts arrives twice, once with each statement. Paired, it is one
	transfer: the balances stay the same and it counts as neither spending nor income.
</p>

{#if loadError}
	<p class="alert" role="alert">
		{loadError} <button class="secondary" onclick={load}>Try again</button>
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
	<p class="hint">Loading…</p>
{/if}
