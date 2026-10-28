<script lang="ts">
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
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
			error = problem(failure, response?.status, 'The accounts could not be loaded');
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
	<title>Import · Coinroost</title>
</svelte:head>

<h1>Import bank files</h1>

{#if error}
	<p class="alert" role="alert">
		{error} <button class="secondary" onclick={load}>Try again</button>
	</p>
{:else if loaded}
	<div class="stack">
		<label class="drop" ondragover={(event) => event.preventDefault()} ondrop={drop}>
			Choose files
			<span class="hint">
				Statements as the bank exports them (CSV, Excel or PDF), one or many at once. Nothing is
				imported until you confirm each file.
			</span>
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
	<p class="hint">Loading…</p>
{/if}

<style>
	.drop {
		border: 2px dashed var(--border);
		border-radius: var(--radius);
		padding: 16px;
		background: var(--surface);
	}
</style>
