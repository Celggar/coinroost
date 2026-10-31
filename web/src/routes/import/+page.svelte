<script lang="ts">
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Icon from '#lib/nav/Icon.svelte';
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import { glide, rise, vanish } from '#lib/motion.ts';
	import FileCard from '#lib/import/FileCard.svelte';

	let accounts = $state<Schemas['Account'][]>([]);
	let error = $state('');
	let loaded = $state(false);
	let over = $state(false);
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
		{#if accounts.length === 0}
			<!-- Every file is read into one of the household's accounts: the first step comes first, and choosing files
			     is not offered until there is an account to read them into. -->
			<EmptyState
				icon="bank"
				text={t('import.no_accounts')}
				href="/accounts"
				action={t('import.create_account')}
			/>
		{:else}
			<!-- The browser's own file control speaks its language and draws its own box: it stays, out of sight, and the
			 whole zone is its label, with a button drawn in the page's style. Files can also be dropped on it. -->
			<label
				class="drop"
				class:over
				ondragover={(event) => {
					event.preventDefault();
					over = true;
				}}
				ondragleave={() => (over = false)}
				ondrop={(event) => {
					over = false;
					drop(event);
				}}
			>
				<span class="icon"><Icon name="import" size={28} /></span>
				<span class="title" id="drop-title">{t('import.drop_title')}</span>
				<span class="hint" id="drop-hint">{t('import.choose_hint')}</span>
				<span class="button pick" id="drop-pick">{t('import.choose')}</span>
				<span class="hint here">{t('import.drop_here')}</span>
				<input
					class="visually-hidden"
					type="file"
					aria-labelledby="drop-pick"
					aria-describedby="drop-hint"
					multiple
					accept=".csv,.xls,.xlsx,.pdf"
					onchange={choose}
				/>
			</label>
		{/if}

		{#each files as entry (entry.id)}
			<div class="file" in:rise out:vanish animate:glide>
				<FileCard
					file={entry.file}
					{accounts}
					onremove={() => {
						files = files.filter((f) => f.id !== entry.id);
						// The card and its button are gone: the focus goes back to choosing files.
						document.querySelector<HTMLInputElement>('.drop input')?.focus();
					}}
				/>
			</div>
		{/each}
	</div>
{:else}
	<p class="hint loading">{t('app.loading')}</p>
{/if}

<style>
	.drop {
		display: grid;
		justify-items: center;
		gap: var(--space-sm);
		padding: var(--space-2xl) var(--space-xl);
		border: 2px dashed var(--field-border);
		border-radius: var(--radius-card);
		background: var(--surface);
		text-align: center;
		cursor: pointer;
		transition:
			border-color var(--duration-short) var(--ease-standard),
			background-color var(--duration-short) var(--ease-standard);
	}
	.drop.over {
		border-color: var(--accent);
	}
	@media (hover: hover) {
		.drop:hover {
			border-color: var(--accent);
		}
	}
	.drop.over {
		background: color-mix(in oklab, var(--accent) 8%, var(--surface));
	}
	/* The hidden control has the focus; the zone shows it. */
	.drop:has(input:focus-visible) {
		border-color: var(--accent);
		outline: 3px solid var(--focus);
		outline-offset: 2px;
	}
	.icon {
		display: grid;
		place-items: center;
		width: 56px;
		height: 56px;
		border-radius: 999px;
		background: color-mix(in oklab, var(--accent) 12%, var(--surface));
		color: var(--accent);
	}
	.title {
		font-size: var(--text-lg);
		font-weight: 700;
	}
	.hint {
		max-width: 480px;
	}
	.pick {
		margin-top: var(--space-sm);
	}
	/* Nothing is dragged on a phone. */
	@media (pointer: coarse) {
		.here {
			display: none;
		}
	}
	.file {
		display: grid;
	}
</style>
