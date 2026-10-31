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
		oncreated
	}: {
		categories: Schemas['Category'][];
		payees: Schemas['Payee'][];
		accounts: Schemas['Account'][];
		oncreated: (rule: Schemas['Rule']) => void;
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

	/** The browser's checks, then the amounts: each must read as money, and the range must not be upside down. */
	function checkAll(form: HTMLFormElement): Errors {
		const found = check(form);
		const low = toMinor(min);
		const high = toMinor(max);
		if (Number.isNaN(low)) found.min ??= t('form.amount');
		if (Number.isNaN(high)) found.max ??= t('form.amount');
		if (low !== null && high !== null && low > high) found.max ??= t('problem.rule_min_above_max');
		// The server refuses a rule that sets nothing; saying it here puts the message on the first of the two fields.
		if (!category && !payee) found.category ??= t('problem.rule_nothing_to_set');
		return found;
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		const form = event.currentTarget as HTMLFormElement;
		error = '';
		errors = checkAll(form);
		if (Object.keys(errors).length) {
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
				await tick();
				focusFirst(form, errors);
			} else {
				error = why.message;
				// The button was disabled while it worked, which drops the focus: it goes back to the button.
				await tick();
				form.querySelector<HTMLButtonElement>('button[type="submit"]')?.focus();
			}
			return;
		}
		notify(t('ruleform.added', { name: data.name, position: data.position }));
		name = text = min = max = category = payee = account = '';
		direction = '';
		oncreated(data);
		// The button was disabled while it worked, which drops the focus: it goes back to the form, ready for the next.
		(form.elements.namedItem('name') as HTMLInputElement | null)?.focus();
	}
</script>

<form
	class="card"
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
	<h2>{t('ruleform.title')}</h2>
	<p class="hint">{t('ruleform.hint')}</p>
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
	<FormError text={error} />
	<div class="actions">
		<button type="submit" disabled={busy}>{t('ruleform.add')}</button>
	</div>
</form>
