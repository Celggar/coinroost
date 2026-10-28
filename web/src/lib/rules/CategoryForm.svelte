<script lang="ts">
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { t } from '#lib/i18n.svelte.ts';

	let { oncreated }: { oncreated: (category: Schemas['Category']) => void } = $props();

	let name = $state('');
	let kind = $state<Schemas['CategoryKind']>('expense');
	let error = $state('');
	let done = $state('');
	let busy = $state(false);

	function clear() {
		error = '';
		done = '';
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = '';
		done = '';
		const {
			data,
			error: failure,
			response
		} = await api.POST('/api/v1/categories', {
			body: { name, kind }
		});
		busy = false;
		if (!data) {
			error = problem(failure, response?.status, t('categoryform.failed'));
			return;
		}
		done = t('categoryform.created', { name: data.name });
		name = '';
		oncreated(data);
	}
</script>

<form class="card" onsubmit={submit} oninput={clear} aria-label={t('categoryform.title')}>
	<h2>{t('categoryform.title')}</h2>
	<div class="fields">
		<label>
			{t('categoryform.name')}
			<input required maxlength="100" bind:value={name} />
		</label>
		<label>
			{t('categoryform.holds')}
			<select bind:value={kind}>
				<option value="expense">{t('categoryform.expense')}</option>
				<option value="income">{t('categoryform.income')}</option>
				<option value="transfer">{t('categoryform.transfer')}</option>
			</select>
		</label>
	</div>
	{#if error}<p class="alert" role="alert">{error}</p>{/if}
	{#if done}<p class="notice" role="status">{done}</p>{/if}
	<div><button type="submit" disabled={busy}>{t('categoryform.create')}</button></div>
</form>

<style>
	.fields {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 220px), 1fr));
		gap: 12px;
	}
</style>
