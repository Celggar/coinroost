<script lang="ts">
	// What a choice can become: the same category for every description with this text, as a rule. The text starts from
	// the description and can be edited; Reglas stays the one place that lists and edits rules, so this only makes one.
	// Once made, the rule's changes to what is already imported are shown here (`Changes`), to apply or leave.
	import { tick, untrack } from 'svelte';
	import { api, failure, type Schemas } from '#lib/api/client.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import { names as namesOf } from '#lib/rules/describe.ts';
	import Changes from '#lib/rules/Changes.svelte';
	import Field from '#lib/ui/Field.svelte';
	import FormError from '#lib/ui/FormError.svelte';
	import { check, focusFirst, without, type Errors } from '#lib/ui/form.ts';
	import { notify } from '#lib/ui/notices.svelte.ts';
	import { ruleText } from '#lib/uncategorized/rule-text.ts';

	let {
		description,
		category,
		categories,
		accounts,
		onclose,
		onapplied
	}: {
		description: string;
		category: string;
		categories: Schemas['Category'][];
		accounts: Schemas['Account'][];
		onclose: () => void;
		onapplied: () => void;
	} = $props();

	let text = $state(untrack(() => ruleText(description)));
	let error = $state('');
	let errors = $state<Errors>({});
	let busy = $state(false);
	let made = $state<ReturnType<typeof namesOf> | null>(null);

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
		busy = true;
		const {
			data,
			error: failed,
			response
		} = await api.POST('/api/v1/rules', {
			body: {
				name: text,
				when: { description_contains: text },
				then: { category_id: category }
			}
		});
		if (!data) {
			busy = false;
			const why = failure(failed, response?.status, t('ruleform.failed'));
			if (why.field === 'when.description_contains') {
				errors = { [why.field]: why.message };
				await tick();
				focusFirst(form, errors);
			} else {
				error = why.message;
			}
			return;
		}
		notify(t('uncategorized.rule_made', { name: data.name }));
		// The changes are named as on Reglas, payees included: another rule may be what moves a transaction.
		const payees = await api.GET('/api/v1/payees');
		made = namesOf(categories, payees.data?.items ?? [], accounts);
		busy = false;
	}
</script>

{#if made}
	<Changes names={made} inline {onapplied} />
{:else}
	<form
		class="offer"
		novalidate
		onsubmit={submit}
		oninput={(event) => {
			errors = without(errors, event);
			error = '';
		}}
		aria-label={t('uncategorized.offer')}
	>
		<Field
			label={t('uncategorized.offer')}
			name="when.description_contains"
			error={errors['when.description_contains']}
		>
			{#snippet children(wiring)}
				<input {...wiring} required maxlength="200" bind:value={text} />
			{/snippet}
		</Field>
		<FormError text={error} />
		<div class="actions">
			<button type="submit" aria-disabled={busy}>{t('uncategorized.make_rule')}</button>
			<button type="button" class="secondary" onclick={onclose}
				>{t('uncategorized.only_this')}</button
			>
		</div>
	</form>
{/if}

<style>
	.offer {
		display: grid;
		gap: var(--space-sm);
		max-width: 560px;
	}
	/* Side by side, the two buttons stand as tall as each other. */
	.actions {
		align-items: stretch;
	}
</style>
