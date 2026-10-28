<script lang="ts">
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import FileCard from '#lib/import/FileCard.svelte';

	let accounts = $state<Schemas['Account'][]>([]);
	let error = $state('');
	let loaded = $state(false);
	let files = $state<{ id: number; file: File }[]>([]);
	let next = 0;

	async function load() {
		error = '';
		const { data, error: failure, response } = await api.GET('/api/v1/accounts');
		if (!data) {
			error = problem(failure, response?.status, t('import.accounts_failed'));
			return;
		}
		accounts = data.items;
		loaded = true;
	}

	function add(list: FileList | null | undefined) {
		for (const file of list ?? []) {
			files.push({ id: next++, file });
		}
	}

	function choose(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		add(input.files);
		input.value = '';
	}

	function drop(event: DragEvent) {
		event.preventDefault();
		add(event.dataTransfer?.files);
	}

	onMount(load);
</script>

<svelte:head>
	<title>{t('app.title', { page: t('nav.import') })}</title>
</svelte:head>

<h1>{t('import.title')}</h1>

{#if error}
	<p class="alert" role="alert">
		{error} <button class="secondary" onclick={load}>{t('app.try_again')}</button>
	</p>
{:else if loaded}
	<div class="stack">
		<label class="drop" ondragover={(event) => event.preventDefault()} ondrop={drop}>
			{t('import.choose')}
			<span class="hint">{t('import.choose_hint')}</span>
			<input type="file" multiple accept=".csv,.xls,.xlsx,.pdf" onchange={choose} />
		</label>

		{#each files as entry (entry.id)}
			<FileCard
				file={entry.file}
				{accounts}
				onremove={() => (files = files.filter((f) => f.id !== entry.id))}
			/>
		{/each}
	</div>
{:else}
	<p class="hint">{t('app.loading')}</p>
{/if}

<style>
	.drop {
		border: 2px dashed var(--border);
		border-radius: var(--radius);
		padding: 16px;
		background: var(--surface);
	}
</style>
