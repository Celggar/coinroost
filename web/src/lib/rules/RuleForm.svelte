<script lang="ts">
	import { tick } from 'svelte';
	import { api, failure, type Schemas } from '#lib/api/client.ts';
	import { toMinor } from '#lib/rules/describe.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import Field from '#lib/ui/Field.svelte';
	import FormError from '#lib/ui/FormError.svelte';
	import { check, focusFirst, without, type Errors } from '#lib/ui/form.ts';
	import { notify } from '#lib/ui/notices.svelte.ts';

	let {
		categories,
		payees,
		accounts,
		sheet = false,
		oncancel,
		oncreated
	}: {
		categories: Schemas['Category'][];
		payees: Schemas['Payee'][];
		accounts: Schemas['Account'][];
		/** Inside a full-screen dialog, which holds the title and the send button. */
		sheet?: boolean;
		oncancel?: () => void;
		oncreated: (rule: Schemas['Rule']) => unknown;
	} = $props();

	let name = $state('');
	let text = $state('');
	let direction = $state<'' | 'in' | 'out'>('');
	let account = $state('');
	let min = $state('');
	let max = $state('');
	let category = $state('');
	// '' none · 'text' the name after the matched text · otherwise a payee id
	let payee = $state('');
	let error = $state('');
	let errors = $state<Errors>({});
	let busy = $state(false);
	let more = $state(false);
	// What the fold holds: an error there opens it, or the field it names could not take the focus.
	const FOLDED = ['direction', 'account', 'min', 'max', 'payee'];

	/** The browser's checks, then the amounts: each must read as money, and the range must not be upside down. */
	function checkAll(form: HTMLFormElement): Errors {
		const found = check(form);
		const low = toMinor(min);
		const high = toMinor(max);
		// A signed amount is money all the same: it gets the API's own words, not «write an amount» (pass 14).
		const wrong = (text: string) =>
			/^[-\u2212]\s*\d+(\.\d{1,2})?$/.test(text.trim())
				? t('problem.rule_negative_amount')
				: t('form.amount');
		if (Number.isNaN(low)) found.min ??= wrong(min);
		if (Number.isNaN(high)) found.max ??= wrong(max);
		if (low !== null && high !== null && low > high) found.max ??= t('problem.rule_min_above_max');
		// The server refuses a rule that sets nothing; saying it here puts the message on the first of the two fields.
		if (!category && !payee) found.category ??= t('problem.rule_nothing_to_set');
		return found;
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (busy) return;
		const form = event.currentTarget as HTMLFormElement;
		error = '';
		errors = checkAll(form);
		if (Object.keys(errors).length) {
			if (FOLDED.some((field) => errors[field])) more = true;
			await tick();
			focusFirst(form, errors);
			return;
		}
		busy = true;
		const {
			data,
			error: failed,
			response
		} = await api.POST('/api/v1/rules', {
			body: {
				name,
				when: {
					// Kept exactly as typed: spaces count.
					description_contains: text === '' ? null : text,
					direction: direction || null,
					account_id: account || null,
					amount_min: toMinor(min),
					amount_max: toMinor(max)
				},
				then: {
					category_id: category || null,
					payee_id: payee && payee !== 'text' ? payee : null,
					payee_from_text: payee === 'text'
				}
			}
		});
		busy = false;
		if (!data) {
			const why = failure(failed, response?.status, t('ruleform.failed'));
			if (why.field && form.elements.namedItem(why.field)) {
				errors = { [why.field]: why.message };
				if (FOLDED.includes(why.field)) more = true;
				await tick();
				focusFirst(form, errors);
			} else {
				error = why.message;
			}
			return;
		}
		notify(t('ruleform.added', { name: data.name, position: data.position }));
		await oncreated(data);
	}
</script>

<form
	class:card={!sheet}
	id="rule-form"
	novalidate
	onsubmit={submit}
	oninput={(event) => {
		errors = without(errors, event);
		// Either of the two answers «a category or a payee», so a payee clears the category's message too. The field
		// is read, not its binding: a select's binding updates on change, after this.
		const target = event.target as HTMLSelectElement;
		if (target.name === 'payee' && target.value && errors.category) {
			const rest = { ...errors };
			delete rest.category;
			errors = rest;
		}
		error = '';
	}}
	aria-label={t('ruleform.title')}
>
	{#if !sheet}<h2>{t('ruleform.title')}</h2>{/if}
	<div class="fields">
		<Field label={t('ruleform.name')} name="name" error={errors.name}>
			{#snippet children(wiring)}
				<input {...wiring} required maxlength="200" bind:value={name} />
			{/snippet}
		</Field>
		<Field
			label={t('ruleform.text')}
			name="when.description_contains"
			hint={t('ruleform.text_hint')}
			error={errors['when.description_contains']}
		>
			{#snippet children(wiring)}
				<input {...wiring} maxlength="200" bind:value={text} />
			{/snippet}
		</Field>
		<Field label={t('ruleform.category')} name="category" error={errors.category}>
			{#snippet children(wiring)}
				<select {...wiring} bind:value={category}>
					<option value="">{t('ruleform.no_category')}</option>
					{#each categories as option (option.id)}
						<option value={option.id}>{option.name}</option>
					{/each}
				</select>
			{/snippet}
		</Field>
	</div>
	<details bind:open={more}>
		<summary>{t('ruleform.more')}</summary>
		<div class="fields folded">
			<Field label={t('ruleform.money')} name="direction" error={errors.direction}>
				{#snippet children(wiring)}
					<select {...wiring} bind:value={direction}>
						<option value="">{t('ruleform.either')}</option>
						<option value="out">{t('ruleform.out')}</option>
						<option value="in">{t('ruleform.in')}</option>
					</select>
				{/snippet}
			</Field>
			<Field label={t('column.account')} name="account" error={errors.account}>
				{#snippet children(wiring)}
					<select {...wiring} bind:value={account}>
						<option value="">{t('ruleform.any_account')}</option>
						{#each accounts as option (option.id)}
							<option value={option.id}>{option.name}</option>
						{/each}
					</select>
				{/snippet}
			</Field>
			<Field label={t('ruleform.min')} name="min" error={errors.min}>
				{#snippet children(wiring)}
					<input {...wiring} inputmode="decimal" placeholder="0.00" bind:value={min} />
				{/snippet}
			</Field>
			<Field label={t('ruleform.max')} name="max" error={errors.max}>
				{#snippet children(wiring)}
					<input {...wiring} inputmode="decimal" placeholder="0.00" bind:value={max} />
				{/snippet}
			</Field>
			<Field label={t('ruleform.payee')} name="payee" error={errors.payee}>
				{#snippet children(wiring)}
					<select {...wiring} bind:value={payee}>
						<option value="">{t('ruleform.no_payee')}</option>
						<option value="text">{t('ruleform.payee_from_text')}</option>
						{#each payees as option (option.id)}
							<option value={option.id}>{option.name}</option>
						{/each}
					</select>
				{/snippet}
			</Field>
		</div>
	</details>
	<p class="hint">{t('ruleform.hint')}</p>
	<FormError text={error} />
	{#if !sheet}
		<div class="actions">
			<!-- aria-disabled, not disabled: a disabled button drops the focus to the page while it works, and it sat there
			     until the page landed (pass 17). -->
			<button type="submit" aria-disabled={busy}>{t('ruleform.add')}</button>
			{#if oncancel}
				<button type="button" class="secondary" onclick={oncancel}>{t('ruleform.cancel')}</button>
			{/if}
		</div>
	{/if}
</form>

<style>
	form {
		display: grid;
		gap: var(--space-lg);
	}
	.folded {
		padding-top: var(--space-md);
	}
	/* Side by side, the two buttons stand as tall as each other (FD6). */
	.actions {
		align-items: stretch;
	}
</style>
