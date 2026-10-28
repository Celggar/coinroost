<script lang="ts">
	import { onMount } from 'svelte';
	import { importFile, previewFile, problem, type Schemas } from '#lib/api/client.ts';
	import { money, span } from '#lib/format.ts';

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
				return 'Found from the file.';
			case 'ambiguous':
				return 'The file fits several accounts: choose one.';
			case 'not_found':
				return importable.length
					? 'No account recognizes this file: choose one.'
					: 'Create an account with a bank profile first.';
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
			error = problem(failure, response?.status, 'The file cannot be read');
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
			error = problem(failure, response?.status, 'The import failed');
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
		<button class="secondary" onclick={onremove} aria-label={`Remove ${file.name}`}>Remove</button>
	</header>

	{#if error}<p class="alert" role="alert">{error}</p>{/if}

	{#if answered}
		<label>
			Account
			<select
				bind:value={accountId}
				onchange={() => read(accountId || undefined)}
				disabled={busy || report !== null}
			>
				<option value="" disabled>Choose the account</option>
				{#each choices as choice (choice.id)}
					<option value={choice.id}>{choice.name}</option>
				{/each}
			</select>
			{#if note}<span class="hint"
					>{note}
					{#if !importable.length}<a href="/accounts">Open Accounts</a>{/if}</span
				>{/if}
		</label>
	{:else if busy}
		<p class="hint">Reading…</p>
	{/if}

	{#if reading && account}
		<dl class="counts">
			<div>
				<dt>Rows read</dt>
				<dd>{reading.rows_read}</dd>
			</div>
			<div>
				<dt>New</dt>
				<dd>{reading.new}</dd>
			</div>
			<div>
				<dt>Already imported</dt>
				<dd>{reading.duplicates}</dd>
			</div>
			<div>
				<dt>Unreadable</dt>
				<dd>{reading.errors}</dd>
			</div>
		</dl>
		{#if reading.first_date && reading.last_date}
			<p class="hint">Dates in the file: {span(reading.first_date, reading.last_date)}</p>
		{/if}

		{#if reading.row_errors.length}
			<section aria-label="Rows that cannot be read">
				<h3>Rows that cannot be read</h3>
				<ul class="errors">
					{#each reading.row_errors as failure (failure.row)}
						<li>Row {failure.row}: {failure.reason}</li>
					{/each}
				</ul>
			</section>
		{/if}

		{#if reading.rows.length}
			<details>
				<summary>Show the {reading.rows.length} transactions</summary>
				<div class="table-wrap">
					<table>
						<thead>
							<tr>
								<th>Date</th>
								<th>Description</th>
								<th class="number">Amount</th>
								<th>Status</th>
							</tr>
						</thead>
						<tbody>
							{#each reading.rows as row (row.external_id)}
								<tr>
									<td>{row.date}</td>
									<td>{row.description}</td>
									<td class="number">{money(row.amount_minor, account.currency)}</td>
									<td>{row.duplicate ? 'Already imported' : 'New'}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			</details>
		{/if}

		{#if report}
			<p class="notice" role="status">
				Added {report.batch.added}; skipped {report.batch.duplicates} already imported.
			</p>
		{:else}
			{#if reading.new === 0}
				<p class="notice" role="status">Every row is already imported: there is nothing to add.</p>
			{/if}
			<div>
				<button onclick={commit} disabled={busy || reading.new === 0}>
					Import {reading.new} new
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
	summary {
		cursor: pointer;
		padding: 8px 0;
		min-height: 44px;
	}
</style>
