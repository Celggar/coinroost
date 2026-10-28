<script lang="ts">
	import { onMount } from 'svelte';
	import { importFile, previewFile, problem, type Schemas } from '#lib/api/client.ts';
	import { day, money, span } from '#lib/format.ts';
	import { plural, t } from '#lib/i18n.svelte.ts';

	let {
		file,
		accounts,
		onremove
	}: { file: File; accounts: Schemas['Account'][]; onremove: () => void } = $props();

	let preview = $state<Schemas['ImportPreview'] | null>(null);
	let accountId = $state('');
	let busy = $state(true);
	let answered = $state(false);
	let error = $state('');
	let report = $state<Schemas['ImportReport'] | null>(null);

	const importable = $derived(accounts.filter((a) => a.profile));
	const account = $derived(accounts.find((a) => a.id === accountId));
	const reading = $derived(preview?.reading ?? null);
	const choices = $derived(
		preview?.detection === 'ambiguous'
			? importable.filter((a) => preview?.candidates.includes(a.id))
			: importable
	);
	const note = $derived.by(() => {
		switch (preview?.detection) {
			case 'found':
				return t('file.found');
			case 'ambiguous':
				return t('file.ambiguous');
			case 'not_found':
				return importable.length ? t('file.not_found') : t('file.no_profile');
			default:
				return '';
		}
	});

	async function read(chosen?: string) {
		busy = true;
		error = '';
		const { data, error: failure, response } = await previewFile(file, chosen);
		busy = false;
		answered = true;
		if (!data) {
			// Keep the choice open: another account may read it.
			preview = preview ? { ...preview, reading: null } : null;
			accountId = chosen ?? '';
			error = problem(failure, response?.status, t('file.unreadable'));
			return;
		}
		preview = data;
		accountId = data.account_id ?? '';
	}

	async function commit() {
		if (!accountId) return;
		busy = true;
		error = '';
		const { data, error: failure, response } = await importFile(file, accountId);
		busy = false;
		if (!data) {
			error = problem(failure, response?.status, t('file.import_failed'));
			return;
		}
		report = data;
		// Read it again so the counts and rows show what the account has now.
		await read(accountId);
	}

	onMount(() => {
		read();
	});
</script>

<article class="card" aria-label={file.name} aria-busy={busy}>
	<header class="head">
		<h2>{file.name}</h2>
		<button
			class="secondary"
			onclick={onremove}
			aria-label={t('file.remove_named', { name: file.name })}>{t('file.remove')}</button
		>
	</header>

	{#if error}<p class="alert" role="alert">{error}</p>{/if}

	{#if answered}
		<label>
			{t('file.account')}
			<select
				bind:value={accountId}
				onchange={() => read(accountId || undefined)}
				disabled={busy || report !== null}
			>
				<option value="" disabled>{t('file.choose_account')}</option>
				{#each choices as choice (choice.id)}
					<option value={choice.id}>{choice.name}</option>
				{/each}
			</select>
			{#if note}<span class="hint"
					>{note}
					{#if !importable.length}<a class="target" href="/accounts">{t('file.open_accounts')}</a
						>{/if}</span
				>{/if}
		</label>
	{:else if busy}
		<p class="hint">{t('file.reading')}</p>
	{/if}

	{#if reading && account}
		<dl class="counts">
			<div>
				<dt>{t('file.rows_read')}</dt>
				<dd>{reading.rows_read}</dd>
			</div>
			<div>
				<dt>{t('file.new')}</dt>
				<dd>{reading.new}</dd>
			</div>
			<div>
				<dt>{t('file.already')}</dt>
				<dd>{reading.duplicates}</dd>
			</div>
			<div>
				<dt>{t('file.unreadable_rows')}</dt>
				<dd>{reading.errors}</dd>
			</div>
		</dl>
		{#if reading.first_date && reading.last_date}
			<p class="hint">
				{t('file.dates', { span: span(reading.first_date, reading.last_date) })}
			</p>
		{/if}

		{#if reading.row_errors.length}
			<section aria-label={t('file.bad_rows')}>
				<h3>{t('file.bad_rows')}</h3>
				<ul class="errors">
					{#each reading.row_errors as failure (failure.row)}
						<li>{t('file.bad_row', { row: failure.row, reason: failure.reason })}</li>
					{/each}
				</ul>
			</section>
		{/if}

		{#if reading.rows.length}
			<details>
				<summary>{plural(reading.rows.length, 'file.show_one', 'file.show_other')}</summary>
				<div class="table-wrap">
					<table>
						<thead>
							<tr>
								<th>{t('column.date')}</th>
								<th>{t('column.description')}</th>
								<th class="number">{t('column.amount')}</th>
								<th>{t('column.status')}</th>
							</tr>
						</thead>
						<tbody>
							{#each reading.rows as row (row.external_id)}
								<tr>
									<td>{day(row.date)}</td>
									<td>{row.description}</td>
									<td class="number">{money(row.amount_minor, account.currency)}</td>
									<td>{row.duplicate ? t('file.row_already') : t('file.row_new')}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			</details>
		{/if}

		{#if report}
			<p class="notice" role="status">
				{t('file.added', { added: report.batch.added, duplicates: report.batch.duplicates })}
			</p>
		{:else}
			{#if reading.new === 0}
				<p class="notice" role="status">{t('file.nothing_new')}</p>
			{/if}
			<div>
				<button onclick={commit} disabled={busy || reading.new === 0}>
					{plural(reading.new, 'file.import_one', 'file.import_other')}
				</button>
			</div>
		{/if}
	{/if}
</article>

<style>
	.head {
		display: flex;
		justify-content: space-between;
		align-items: start;
		gap: 12px;
	}
	.head h2 {
		min-width: 0;
	}
	.counts {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
		gap: 8px;
		margin: 0;
	}
	.counts div {
		background: var(--bg);
		border-radius: var(--radius);
		padding: 8px 10px;
	}
	dt {
		color: var(--muted);
		font-size: 0.85rem;
	}
	dd {
		margin: 0;
		font-size: 1.25rem;
		font-weight: 700;
		font-variant-numeric: tabular-nums;
	}
	h3 {
		font-size: 1rem;
		margin: 0 0 4px;
	}
	.errors {
		margin: 0;
		padding-left: 20px;
		color: var(--bad);
	}
	/* A 24 px target at least (WCAG 2.5.8), though it sits in a hint. */
	.target {
		display: inline-block;
		padding: 4px 0;
		min-height: 24px;
	}
	summary {
		cursor: pointer;
		padding: 8px 0;
		min-height: 44px;
	}
</style>
