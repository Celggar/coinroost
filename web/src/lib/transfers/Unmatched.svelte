<script lang="ts">
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { legText, reasonLabel, type AccountNames } from '#lib/transfers/legs.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import { vanish } from '#lib/motion.ts';
	import Field from '#lib/ui/Field.svelte';
	import FormError from '#lib/ui/FormError.svelte';
	import { check, focusFirst, without, type Errors } from '#lib/ui/form.ts';
	import { notify } from '#lib/ui/notices.svelte.ts';

	let {
		preview,
		names,
		onchanged
	}: { preview: Schemas['TransferPreview']; names: AccountNames; onchanged: () => void } = $props();

	let first = $state('');
	let second = $state('');
	let error = $state('');
	let errors = $state<Errors>({});
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

	/** Pairs the two; the reason it cannot comes back, or '' once they are one transfer. */
	async function pair(a: string, b: string): Promise<string> {
		busy = true;
		const {
			data,
			error: failure,
			response
		} = await api.POST('/api/v1/transfers', { body: { transaction_ids: [a, b] } });
		busy = false;
		if (!data) return problem(failure, response?.status, t('unmatched.failed'));
		notify(t('unmatched.paired'));
		first = second = '';
		onchanged();
		return '';
	}

	async function suggested(a: string, b: string) {
		const why = await pair(a, b);
		if (why) notify(why, 'bad');
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		const form = event.currentTarget as HTMLFormElement;
		error = '';
		errors = check(form);
		if (Object.keys(errors).length) {
			focusFirst(form, errors);
			return;
		}
		error = await pair(first, second);
	}
</script>

<section class="card" aria-labelledby="unmatched-title">
	<h2 id="unmatched-title">{t('unmatched.title')}</h2>
	{#if preview.unmatched.length === 0}
		<p>{t('unmatched.nothing')}</p>
	{:else}
		<ul class="legs" aria-label={t('unmatched.title')}>
			{#each preview.unmatched as item (item.leg.transaction_id)}
				<li out:vanish>
					<div class="what">{legText(item.leg, names)}</div>
					<div class="hint">{reasonLabel(item.reason)}</div>
					{#each item.candidates as candidate (candidate.transaction_id)}
						<button
							class="secondary"
							disabled={busy}
							onclick={() => suggested(item.leg.transaction_id, candidate.transaction_id)}
							>{t('unmatched.pair_with', { leg: legText(candidate, names) })}</button
						>
					{/each}
				</li>
			{/each}
		</ul>
	{/if}

	<form
		novalidate
		onsubmit={submit}
		onchange={(event) => {
			errors = without(errors, event);
			error = '';
		}}
		aria-label={t('unmatched.by_hand')}
	>
		<h3>{t('unmatched.by_hand')}</h3>
		<p class="hint">{t('unmatched.by_hand_hint')}</p>
		<div class="fields">
			<Field label={t('unmatched.one_side')} name="first" error={errors.first}>
				{#snippet children(wiring)}
					<select {...wiring} bind:value={first} required>
						<option value="" disabled>{t('unmatched.choose')}</option>
						{#each choices as leg (leg.transaction_id)}
							<option value={leg.transaction_id}>{legText(leg, names)}</option>
						{/each}
					</select>
				{/snippet}
			</Field>
			<Field label={t('unmatched.other_side')} name="second" error={errors.second}>
				{#snippet children(wiring)}
					<select {...wiring} bind:value={second} required>
						<option value="" disabled>{t('unmatched.choose')}</option>
						{#each choices as leg (leg.transaction_id)}
							<option value={leg.transaction_id}>{legText(leg, names)}</option>
						{/each}
					</select>
				{/snippet}
			</Field>
		</div>
		<FormError text={error} />
		<div class="actions">
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
		gap: var(--space-sm);
	}
	.legs li {
		border-top: 1px solid var(--border);
		padding-top: var(--space-sm);
		display: grid;
		gap: var(--space-sm);
		justify-items: start;
	}
	.what,
	button {
		overflow-wrap: anywhere;
		text-align: start;
	}
	form {
		display: grid;
		gap: var(--space-md);
		border-top: 1px solid var(--border);
		padding-top: var(--space-lg);
	}
	select {
		max-width: 100%;
	}
</style>
