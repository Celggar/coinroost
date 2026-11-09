<script lang="ts">
	import { tick } from 'svelte';
	import { api, failure, type Schemas } from '#lib/api/client.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import { holds } from '#lib/ui/CategoryOptions.svelte';
	import Field from '#lib/ui/Field.svelte';
	import FormError from '#lib/ui/FormError.svelte';
	import { check, focusFirst, without, type Errors } from '#lib/ui/form.ts';
	import { landOn } from '#lib/ui/landing.ts';
	import { notify } from '#lib/ui/notices.svelte.ts';
	import { pop } from '#lib/motion.ts';

	// `back`: the person came from the uncategorized queue to make the categories it lacked; once one exists, the way
	// back to the queue shows by the button, or they had to know to tap «Revisar» (pass 10).
	let {
		oncreated,
		back = false
	}: { oncreated: (category: Schemas['Category']) => unknown; back?: boolean } = $props();
	let made = $state(false);

	let name = $state('');
	let kind = $state<Schemas['CategoryKind']>('expense');
	let error = $state('');
	let errors = $state<Errors>({});
	let busy = $state(false);

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
		made = true;
		name = '';
		// Back to the form for the next one once the page has reloaded and settled (as the rule form does).
		const field = form.elements.namedItem('name') as HTMLInputElement | null;
		const left = document.activeElement;
		await oncreated(data);
		if (field) await landOn(field, form, undefined, left);
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
	id="new-category"
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
					{#each Object.entries(holds) as [value, key] (value)}
						<option {value}>{t(key)}</option>
					{/each}
				</select>
			{/snippet}
		</Field>
	</div>
	<FormError text={error} />
	<div class="actions">
		<!-- aria-disabled, not disabled: a disabled button drops the focus to the page while it works, and it sat there
		     until the page landed (pass 17). -->
		<button type="submit" aria-disabled={busy}>{t('categoryform.create')}</button>
		{#if back && made}<a class="button secondary" href="/uncategorized" in:pop
				>{t('categoryform.back')}</a
			>{/if}
	</div>
</form>

<style>
	/* A row with the primary in it: the way back takes the primary's height (FD6), so the row shares its edges. */
	.actions .button {
		min-height: var(--primary-height);
	}
</style>
