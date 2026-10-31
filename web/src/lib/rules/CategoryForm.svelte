<script lang="ts">
	import { tick } from 'svelte';
	import { api, failure, type Schemas } from '#lib/api/client.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import Field from '#lib/ui/Field.svelte';
	import FormError from '#lib/ui/FormError.svelte';
	import { check, focusFirst, without, type Errors } from '#lib/ui/form.ts';
	import { notify } from '#lib/ui/notices.svelte.ts';

	let { oncreated }: { oncreated: (category: Schemas['Category']) => void } = $props();

	let name = $state('');
	let kind = $state<Schemas['CategoryKind']>('expense');
	let error = $state('');
	let errors = $state<Errors>({});
	let busy = $state(false);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
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
		} = await api.POST('/api/v1/categories', {
			body: { name, kind }
		});
		busy = false;
		if (!data) {
			const why = failure(failed, response?.status, t('categoryform.failed'));
			if (why.field && form.elements.namedItem(why.field)) {
				errors = { [why.field]: why.message };
				await tick();
				focusFirst(form, errors);
			} else {
				error = why.message;
			}
			return;
		}
		notify(t('categoryform.created', { name: data.name }));
		name = '';
		oncreated(data);
		(form.elements.namedItem('name') as HTMLInputElement | null)?.focus();
	}
</script>

<form
	class="card"
	novalidate
	onsubmit={submit}
	oninput={(event) => {
		errors = without(errors, event);
		error = '';
	}}
	aria-label={t('categoryform.title')}
>
	<h2>{t('categoryform.title')}</h2>
	<div class="fields">
		<Field label={t('categoryform.name')} name="name" error={errors.name}>
			{#snippet children(wiring)}
				<input {...wiring} required maxlength="100" bind:value={name} />
			{/snippet}
		</Field>
		<Field label={t('categoryform.holds')} name="kind" error={errors.kind}>
			{#snippet children(wiring)}
				<select {...wiring} bind:value={kind}>
					<option value="expense">{t('categoryform.expense')}</option>
					<option value="income">{t('categoryform.income')}</option>
					<option value="transfer">{t('categoryform.transfer')}</option>
				</select>
			{/snippet}
		</Field>
	</div>
	<FormError text={error} />
	<div class="actions">
		<button type="submit" disabled={busy}>{t('categoryform.create')}</button>
	</div>
</form>
