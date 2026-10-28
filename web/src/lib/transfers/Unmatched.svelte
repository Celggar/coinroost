<script lang="ts">
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { legText, reasonLabel, type AccountNames } from '#lib/transfers/legs.ts';
	import { t } from '#lib/i18n.svelte.ts';

	let {
		preview,
		names,
		onchanged
	}: { preview: Schemas['TransferPreview']; names: AccountNames; onchanged: () => void } = $props();

	let first = $state('');
	let second = $state('');
	let error = $state('');
	let done = $state('');
	let busy = $state(false);

	// Every leg still waiting, paired in the preview or not: by hand, any two can be tried.
	const choices = $derived(
		[
			...preview.pairs.flatMap((p) => [p.outflow, p.inflow]),
			...preview.unmatched.map((u) => u.leg)
		].sort(
			(a, b) => a.date.localeCompare(b.date) || a.transaction_id.localeCompare(b.transaction_id)
		)
	);

	async function pair(a: string, b: string) {
		busy = true;
		error = '';
		done = '';
		const {
			data,
			error: failure,
			response
		} = await api.POST('/api/v1/transfers', { body: { transaction_ids: [a, b] } });
		busy = false;
		if (!data) {
			error = problem(failure, response?.status, t('unmatched.failed'));
			return;
		}
		done = t('unmatched.paired');
		first = second = '';
		onchanged();
	}

	// A new choice makes the last answer stale.
	function clear() {
		error = '';
		done = '';
	}

	function submit(event: SubmitEvent) {
		event.preventDefault();
		if (first && second) pair(first, second);
	}
</script>

<section class="card" aria-labelledby="unmatched-title">
	<h2 id="unmatched-title">{t('unmatched.title')}</h2>
	{#if preview.unmatched.length === 0}
		<p>{t('unmatched.nothing')}</p>
	{:else}
		<ul class="legs" aria-label={t('unmatched.title')}>
			{#each preview.unmatched as item (item.leg.transaction_id)}
				<li>
					<div class="what">{legText(item.leg, names)}</div>
					<div class="hint">{reasonLabel(item.reason)}</div>
					{#each item.candidates as candidate (candidate.transaction_id)}
						<button
							class="secondary"
							disabled={busy}
							onclick={() => pair(item.leg.transaction_id, candidate.transaction_id)}
							>{t('unmatched.pair_with', { leg: legText(candidate, names) })}</button
						>
					{/each}
				</li>
			{/each}
		</ul>
	{/if}

	<form onsubmit={submit} onchange={clear} aria-label={t('unmatched.by_hand')}>
		<h3>{t('unmatched.by_hand')}</h3>
		<p class="hint">{t('unmatched.by_hand_hint')}</p>
		<div class="fields">
			<label>
				{t('unmatched.one_side')}
				<select bind:value={first} required>
					<option value="" disabled>{t('unmatched.choose')}</option>
					{#each choices as leg (leg.transaction_id)}
						<option value={leg.transaction_id}>{legText(leg, names)}</option>
					{/each}
				</select>
			</label>
			<label>
				{t('unmatched.other_side')}
				<select bind:value={second} required>
					<option value="" disabled>{t('unmatched.choose')}</option>
					{#each choices as leg (leg.transaction_id)}
						<option value={leg.transaction_id}>{legText(leg, names)}</option>
					{/each}
				</select>
			</label>
		</div>
		{#if error}<p class="alert" role="alert">{error}</p>{/if}
		{#if done}<p class="notice" role="status">{done}</p>{/if}
		<div>
			<button type="submit" disabled={busy || choices.length < 2}>{t('unmatched.pair_them')}</button
			>
		</div>
	</form>
</section>

<style>
	.legs {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 8px;
	}
	.legs li {
		border-top: 1px solid var(--border);
		padding-top: 8px;
		display: grid;
		gap: 6px;
		justify-items: start;
	}
	.what,
	button {
		overflow-wrap: anywhere;
		text-align: start;
	}
	form {
		display: grid;
		gap: 10px;
		border-top: 1px solid var(--border);
		padding-top: 12px;
	}
	h3 {
		margin: 0;
		font-size: 1rem;
	}
	.fields {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 260px), 1fr));
		gap: 12px;
		align-items: end;
	}
	select {
		max-width: 100%;
	}
</style>
