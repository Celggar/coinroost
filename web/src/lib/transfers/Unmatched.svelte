<script lang="ts">
	import { tick } from 'svelte';
	import { refreshCounts } from '#lib/nav/counts.svelte.ts';
	import { api, failure, type Failure, type Schemas } from '#lib/api/client.ts';
	import { day } from '#lib/format.ts';
	import { accountName, legText, reasonLabel, type AccountNames } from '#lib/transfers/legs.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import { followContent, glide, rise, swapIn, vanish } from '#lib/motion.ts';
	import Field from '#lib/ui/Field.svelte';
	import FormError from '#lib/ui/FormError.svelte';
	import Money from '#lib/ui/Money.svelte';
	import { check, focusFirst, without, type Errors } from '#lib/ui/form.ts';
	import { showWhole } from '#lib/ui/landing.ts';
	import { focusOn } from '#lib/ui/focus.ts';
	import { notify } from '#lib/ui/notices.svelte.ts';

	let {
		preview,
		names,
		onchanged
	}: { preview: Schemas['TransferPreview']; names: AccountNames; onchanged: () => Promise<void> } =
		$props();

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
		// The focus moves once the page has reloaded and the lists have their new size.
		await onchanged();
		await tick();
		return null;
	}

	// The button or the choices that made the pair go away with it: the focus waits on this card's title, and the page
	// glides to it once the lists have their new size (focused bare, the page jumped in one frame: pass 15).
	const land = async () => {
		const title = focusOn('#unmatched-title');
		if (title) await showWhole(title);
	};

	async function suggested(a: string, b: string) {
		const why = await pair(a, b);
		if (why) notify(why.message, 'bad');
		else land();
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (busy) return;
		const form = event.currentTarget as HTMLFormElement;
		error = '';
		errors = check(form);
		if (Object.keys(errors).length) {
			focusFirst(form, errors);
			return;
		}
		const why = await pair(first, second);
		if (!why) {
			land();
			return;
		}
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

<!-- The card shrinks as the pairing form leaves instead of at once: at the page's end it went 490 → 178 px in one
     frame and pulled the page back 232 px (pass 16 sweep). -->
<section class="card follows" aria-labelledby="unmatched-title" use:followContent>
	<div class="stack">
		<h2 id="unmatched-title" tabindex="-1">{t('unmatched.title')}</h2>
		<div class="in-place">
			{#if preview.unmatched.length === 0}
				<p in:swapIn out:vanish>{t('unmatched.nothing')}</p>
			{:else}
				<ul class="legs txn-rows" aria-label={t('unmatched.title')} in:swapIn out:vanish>
					{#each preview.unmatched as item (item.leg.transaction_id)}
						<li in:rise out:vanish animate:glide>
							<div class="txn-line">
								<span>{day(item.leg.date)}</span>
								<span class="description">{accountName(item.leg.account_id, names)}</span>
								<Money minor={item.leg.amount_minor} currency={item.leg.currency} />
							</div>
							<div class="hint">{item.leg.description}</div>
							<div class="hint">{reasonLabel(item.reason)}</div>
							<!-- Each possible other side reads like a leg, inset, with a short button beside it (under it on a
							     phone); the button's name says which leg it pairs with. -->
							{#if item.candidates.length}
								<ul class="candidates">
									{#each item.candidates as candidate (candidate.transaction_id)}
										<li>
											<div class="candidate">
												<div class="txn-line">
													<span>{day(candidate.date)}</span>
													<span class="description">{accountName(candidate.account_id, names)}</span
													>
													<Money minor={candidate.amount_minor} currency={candidate.currency} />
												</div>
												<div class="hint">{candidate.description}</div>
											</div>
											<button
												class="secondary"
												aria-disabled={busy}
												aria-label={t('unmatched.pair_with', { leg: legText(candidate, names) })}
												onclick={() =>
													busy || suggested(item.leg.transaction_id, candidate.transaction_id)}
												>{t('unmatched.pair')}</button
											>
										</li>
									{/each}
								</ul>
							{/if}
						</li>
					{/each}
				</ul>
			{/if}
		</div>

		<!-- Pairing by hand needs two legs to choose from; with fewer it is not offered. -->
		{#if choices.length >= 2}
			<form
				in:rise
				out:vanish
				novalidate
				onsubmit={submit}
				onchange={(event) => {
					errors = without(errors, event);
					error = '';
					// The leg chosen on one side is not offered on the other (pass 19): one chosen there before goes.
					if (second === first) second = '';
				}}
				aria-label={t('unmatched.by_hand')}
			>
				<h3>{t('unmatched.by_hand')}</h3>
				<p class="hint">{t('unmatched.by_hand_hint')}</p>
				<div class="fields pick">
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
									<option value={leg.transaction_id} disabled={leg.transaction_id === first}
										>{legText(leg, names)}</option
									>
								{/each}
							</select>
						{/snippet}
					</Field>
				</div>
				<FormError text={error} />
				<div class="actions">
					<button type="submit" aria-disabled={busy}>{t('unmatched.pair_them')}</button>
				</div>
			</form>
		{/if}
	</div>
</section>

<style>
	/* Clipped at the padding's edge, the buttons' focus rings stay whole. */
	.follows {
		position: relative;
		overflow: clip;
		align-content: start;
	}
	@media (prefers-reduced-motion: no-preference) {
		.follows {
			transition: height var(--duration-medium) var(--ease-standard);
		}
	}
	/* One choice per line: a leg's text is long, and a half-width select cut it mid-word (pass 1). */
	.pick {
		grid-template-columns: minmax(0, 1fr);
	}
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
		gap: var(--space-sm) var(--space-md);
		justify-items: start;
	}
	/* The buttons keep their own width; the line spans the row, so its amount ends at the edge like every list's. */
	.legs .txn-line {
		justify-self: stretch;
	}
	.candidates {
		list-style: none;
		margin: 0;
		padding: 0;
		justify-self: stretch;
		display: grid;
		gap: var(--space-sm);
	}
	.candidates li {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-sm) var(--space-lg);
		padding: var(--space-sm) var(--space-md);
		border-radius: var(--radius-button);
		background: var(--bg);
	}
	.candidate {
		flex: 1 1 240px;
		min-width: 0;
		display: grid;
		gap: var(--space-2xs);
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
