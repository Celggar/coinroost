<script lang="ts">
	import { refreshCounts } from '#lib/nav/counts.svelte.ts';
	import { api, failure, type Failure, type Schemas } from '#lib/api/client.ts';
	import { day } from '#lib/format.ts';
	import { accountName, legText, reasonLabel, type AccountNames } from '#lib/transfers/legs.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import { glide, rise, vanish } from '#lib/motion.ts';
	import Field from '#lib/ui/Field.svelte';
	import FormError from '#lib/ui/FormError.svelte';
	import Money from '#lib/ui/Money.svelte';
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

	/** Why a pair was not made, and whether the API refused the two chosen (422) rather than failing to answer. */
	type Refusal = Failure & { refused: boolean };

	/** Pairs the two; why it cannot comes back, or null once they are one transfer. */
	async function pair(a: string, b: string): Promise<Refusal | null> {
		busy = true;
		const {
			data,
			error: problem,
			response
		} = await api.POST('/api/v1/transfers', { body: { transaction_ids: [a, b] } });
		busy = false;
		if (!data) {
			return {
				...failure(problem, response?.status, t('unmatched.failed')),
				refused: response?.status === 422
			};
		}
		notify(t('unmatched.paired'));
		refreshCounts();
		first = second = '';
		onchanged();
		return null;
	}

	async function suggested(a: string, b: string) {
		const why = await pair(a, b);
		if (why) notify(why.message, 'bad');
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
		const why = await pair(first, second);
		if (!why) return;
		if (!why.refused) {
			error = why.message;
			return;
		}
		// A refusal is about the second choice next to the first: its message goes under «Otro lado», which takes
		// the focus, as a check of one field does.
		errors = { second: why.message };
		focusFirst(form, errors);
	}
</script>

<section class="card" aria-labelledby="unmatched-title">
	<h2 id="unmatched-title">{t('unmatched.title')}</h2>
	<div class="swap">
		{#if preview.unmatched.length === 0}
			<p in:rise out:vanish>{t('unmatched.nothing')}</p>
		{:else}
			<ul class="legs" aria-label={t('unmatched.title')} in:rise out:vanish>
				{#each preview.unmatched as item (item.leg.transaction_id)}
					<li in:rise out:vanish animate:glide>
						<div class="what">
							<span>{day(item.leg.date)}</span>
							<span>{accountName(item.leg.account_id, names)}</span>
							<Money minor={item.leg.amount_minor} currency={item.leg.currency} />
							<span class="description">{item.leg.description}</span>
						</div>
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
	</div>

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
	.what {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-xs) var(--space-md);
	}
	.description,
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
