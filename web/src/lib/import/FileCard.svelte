<script lang="ts">
	import { refreshCounts } from '#lib/nav/counts.svelte.ts';
	import { onMount, tick } from 'svelte';
	import { importFile, previewFile, problem, type Schemas } from '#lib/api/client.ts';
	import { day, money, span } from '#lib/format.ts';
	import { has, plural, t } from '#lib/i18n.svelte.ts';
	import { lift, rise, vanish } from '#lib/motion.ts';
	import Field from '#lib/ui/Field.svelte';
	import Money from '#lib/ui/Money.svelte';
	import Tick from '#lib/ui/Tick.svelte';
	import { scrollable } from '#lib/ui/scrollable.ts';
	import { notify } from '#lib/ui/notices.svelte.ts';

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
	// Why the chosen account cannot read the file: it goes under «Cuenta», in place of the hint (FD5).
	let accountError = $state('');
	let resultLine = $state<HTMLElement>();
	let report = $state<Schemas['ImportReport'] | null>(null);

	const importable = $derived(accounts.filter((a) => a.profile));
	const account = $derived(accounts.find((a) => a.id === accountId));
	const reading = $derived(preview?.reading ?? null);
	const layoutRows = $derived(
		reading ? reading.rows_read - reading.new - reading.duplicates - reading.errors : 0
	);

	/** What an import did, in one sentence; the already-imported part only when there was some. */
	function added(count: number, duplicates: number): string {
		const first = plural(count, 'file.added_one', 'file.added_other');
		return duplicates
			? `${first} ${plural(duplicates, 'file.skipped_one', 'file.skipped_other')}`
			: first;
	}
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

	// Each read is numbered: the answer of an earlier choice that comes back late is dropped.
	let reads = 0;

	async function read(chosen?: string) {
		const mine = ++reads;
		busy = true;
		error = '';
		const { data, error: failure, response } = await previewFile(file, chosen);
		if (mine !== reads) return;
		busy = false;
		answered = true;
		if (!data) {
			// Keep the choice open: another account may read it.
			preview = preview ? { ...preview, reading: null } : null;
			accountId = chosen ?? '';
			const why = problem(failure, response?.status, t('file.unreadable'));
			if (chosen) accountError = why;
			else error = why;
			return;
		}
		accountError = '';
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
		notify(added(data.batch.added, data.batch.duplicates));
		refreshCounts();
		// The import button is gone: the focus goes to what it did, not to the page's start. The card stays the record
		// of this import (its counts and rows say what it added), so the file is not read again.
		await tick();
		resultLine?.focus();
	}

	onMount(() => {
		read();
	});
	/** Why a row could not be read, in the page's language: the text of its code, else the API's English. */
	function rowReason(failure: Schemas['RowFailure']): string {
		const key = `row.${failure.code}`;
		return has(key) ? t(key, { value: failure.value ?? '' }) : failure.reason;
	}
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

	{#if error}<p class="alert" role="alert" in:rise out:lift>{error}</p>{/if}

	{#if answered}
		<!-- No account reads the file, or none recognised it: the way on is an account that does, made on Cuentas. It
		     stays under the error of an account that cannot read it, or only «Quitar» was left (pass 10). -->
		{@const wayOn = !importable.length || preview?.detection === 'not_found'}
		{#snippet open()}<a class="target" href="/accounts">{t('file.open_accounts')}</a>{/snippet}
		{#snippet found()}
			{note}
			{#if wayOn}{@render open()}{/if}
		{/snippet}
		<Field
			label={t('file.account')}
			name="account"
			hint={accountError ? (wayOn ? open : '') : note ? found : ''}
			error={accountError}
		>
			{#snippet children(wiring)}
				<select
					{...wiring}
					bind:value={accountId}
					onchange={() => read(accountId || undefined)}
					disabled={report !== null}
				>
					<option value="" disabled>{t('file.choose_account')}</option>
					{#each choices as choice (choice.id)}
						<option value={choice.id}>{choice.name}</option>
					{/each}
				</select>
			{/snippet}
		</Field>
	{:else if busy}
		<p class="hint" in:rise out:vanish>{t('file.reading')}</p>
	{/if}

	<!-- What the chosen account read, as one block: it rises in, and fades out when another account cannot read the
		 file, instead of leaving in one frame (FD9, pass 3). -->
	{#if reading && account}
		<div class="reading" in:rise out:vanish>
			<dl class="counts">
				<div>
					<dt>{t('file.rows_read')}</dt>
					<dd>
						<Tick value={reading.rows_read}>{reading.rows_read}</Tick>
					</dd>
				</div>
				<div>
					<dt>
						<Tick value={Boolean(report)}>{report ? t('file.added') : t('file.new')}</Tick>
					</dt>
					<dd>
						<Tick value={report ? report.batch.added : reading.new}
							>{report ? report.batch.added : reading.new}</Tick
						>
					</dd>
				</div>
				<div>
					<dt>{t('file.already')}</dt>
					<dd>
						<Tick value={report ? report.batch.duplicates : reading.duplicates}
							>{report ? report.batch.duplicates : reading.duplicates}</Tick
						>
					</dd>
				</div>
				<div>
					<dt>{t('file.unreadable_rows')}</dt>
					<dd>
						<Tick value={reading.errors}>{reading.errors}</Tick>
					</dd>
				</div>
			</dl>
			<!-- The normalizer's layout rows (blank, header or footer lines, an opening balance) are read but are not
			 transactions: said, so the four counts add up to the rows read. -->
			{#if layoutRows > 0}
				<p class="hint">{plural(layoutRows, 'file.layout_one', 'file.layout_other')}</p>
			{/if}
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
							<li>{t('file.bad_row', { row: failure.row, reason: rowReason(failure) })}</li>
						{/each}
					</ul>
				</section>
			{/if}

			{#if reading.rows.length}
				<details>
					<summary>{plural(reading.rows.length, 'file.show_one', 'file.show_other')}</summary>
					<div class="table-wrap" use:scrollable={t('file.rows')}>
						<table class="rows-on-phone">
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
										<td data-label={t('column.date')}>{day(row.date)}</td>
										<td data-label={t('column.description')}>{row.description}</td>
										<td data-label={t('column.amount')} class="number"
											><Money minor={row.amount_minor} currency={account.currency} /></td
										>
										<td data-label={t('column.status')}
											><Tick value={Boolean(report)}
												>{row.duplicate
													? t('file.row_already')
													: report
														? t('file.row_added')
														: t('file.row_new')}</Tick
											></td
										>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				</details>
			{/if}

			<!-- The card keeps its state (imported); what the import did is said once, by the page's notice (FD8). -->
			{#if report}
				<p class="result" tabindex="-1" bind:this={resultLine} in:rise>
					<span class="check" aria-hidden="true"></span>
					{t('file.imported')}
				</p>
			{:else}
				<!-- Nothing new: the notice says so, and no button is shown that could not be pressed. -->
				{#if reading.new === 0}
					<p class="notice" role="status" in:rise>{t('file.nothing_new')}</p>
				{:else}
					<div class="actions" out:vanish>
						<button onclick={commit} disabled={busy}>
							{plural(reading.new, 'file.import_one', 'file.import_other')}
						</button>
					</div>
				{/if}
			{/if}
		</div>
	{/if}
</article>

<style>
	/* The file's name and «Quitar» share their middle line; a long name wraps beside the button. */
	.head {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--space-md);
	}
	.head h2 {
		min-width: 0;
	}
	/* Its parts keep the card's rhythm. */
	.reading {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-lg);
	}
	.counts {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
		gap: var(--space-sm);
		margin: 0;
	}
	.counts div {
		background: var(--bg);
		border-radius: var(--radius-button);
		padding: var(--space-sm) var(--space-md);
	}
	dt {
		color: var(--muted);
		font-size: var(--text-sm);
	}
	dd {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 700;
		font-variant-numeric: tabular-nums;
	}
	h3 {
		margin: 0 0 var(--space-xs);
	}
	.errors {
		margin: 0;
		padding-left: var(--space-xl);
		color: var(--bad);
	}
	/* A link that is a tap target of its own, 44 px tall, inside a line of text: the padding of an inline box widens
	   what takes the tap but not the line, which grew to 44 px and left a blank line under the hint (pass 11). */
	.target {
		padding-block: calc((44px - 1lh) / 2);
	}
	/* The file's import, done: a check and what it added. */
	/* As tall as the button it replaces: 28 px shorter, it shrank a page scrolled to its end, the browser pulled the page
	   back and left the text above cut across the top bar (pass 13). */
	.result {
		min-height: var(--primary-height);
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		color: var(--ok);
		font-weight: 600;
	}
	.check {
		flex: none;
		width: 20px;
		height: 20px;
		border-radius: 50%;
		background: var(--ok-bg);
		position: relative;
	}
	.check::after {
		content: '';
		position: absolute;
		left: 7px;
		top: 4px;
		width: 5px;
		height: 9px;
		border-right: 2px solid var(--ok);
		border-bottom: 2px solid var(--ok);
		transform: rotate(45deg);
	}
</style>
