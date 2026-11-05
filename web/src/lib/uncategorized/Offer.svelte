<script lang="ts">
	// What a choice can become: the same category for every description with this text, as a rule. The text starts from
	// the description and can be edited; Reglas stays the one place that lists and edits rules, so this only makes one.
	// Once made, the rule's changes to what is already imported are shown here (`Changes`), to apply or leave.
	import { tick, untrack } from 'svelte';
	import { api, failure, type Schemas } from '#lib/api/client.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import { followContent, swapOut } from '#lib/motion.ts';
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
	let answered = $state(false);

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
	}
</script>

<!-- One cell: the form stays, busy and with the focus, until the changes have their answer. Gone first, the row went 197, 73
     and 290 px tall in three frames and the focus fell to the page (pass 41). The room follows what the cell holds. -->
<div class="fit" use:followContent>
	<div class="in-place">
		{#if !answered}
			<form
				class="offer"
				out:swapOut
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
		{#if made}
			<Changes names={made} inline {onapplied} onanswer={() => (answered = true)} />
		{/if}
	</div>
</div>

<style>
	/* Clipped only while it glides: at rest a focus ring reaches past the box. */
	.fit {
		position: relative;
	}
	.fit[data-moving] {
		overflow: clip;
	}
	@media (prefers-reduced-motion: no-preference) {
		.fit {
			transition: height var(--duration-medium) var(--ease-standard);
		}
	}
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
