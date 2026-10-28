<script lang="ts">
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { toMinor } from '#lib/rules/describe.ts';
	import { t } from '#lib/i18n.svelte.ts';

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
	let done = $state('');
	let busy = $state(false);

	function clear() {
		error = '';
		done = '';
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		error = '';
		done = '';
		const amountMin = toMinor(min);
		const amountMax = toMinor(max);
		if (Number.isNaN(amountMin) || Number.isNaN(amountMax)) {
			error = t('ruleform.bad_amount');
			return;
		}
		busy = true;
		const {
			data,
			error: failure,
			response
		} = await api.POST('/api/v1/rules', {
			body: {
				name,
				when: {
					// Kept exactly as typed: spaces count.
					description_contains: text === '' ? null : text,
					direction: direction || null,
					account_id: account || null,
					amount_min: amountMin,
					amount_max: amountMax
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
			error = problem(failure, response?.status, t('ruleform.failed'));
			return;
		}
		done = t('ruleform.added', { name: data.name, position: data.position });
		name = text = min = max = category = payee = account = '';
		direction = '';
		oncreated(data);
	}
</script>

<form class="card" onsubmit={submit} oninput={clear} aria-label={t('ruleform.title')}>
	<h2>{t('ruleform.title')}</h2>
	<p class="hint">{t('ruleform.hint')}</p>
	<div class="fields">
		<label>
			{t('ruleform.name')}
			<input required maxlength="200" bind:value={name} />
		</label>
		<label>
			{t('ruleform.text')}
			<span class="hint">{t('ruleform.text_hint')}</span>
			<input maxlength="200" bind:value={text} />
		</label>
		<label>
			{t('ruleform.money')}
			<select bind:value={direction}>
				<option value="">{t('ruleform.either')}</option>
				<option value="out">{t('ruleform.out')}</option>
				<option value="in">{t('ruleform.in')}</option>
			</select>
		</label>
		<label>
			{t('column.account')}
			<select bind:value={account}>
				<option value="">{t('ruleform.any_account')}</option>
				{#each accounts as option (option.id)}
					<option value={option.id}>{option.name}</option>
				{/each}
			</select>
		</label>
		<label>
			{t('ruleform.min')}
			<input inputmode="decimal" placeholder="0.00" bind:value={min} />
		</label>
		<label>
			{t('ruleform.max')}
			<input inputmode="decimal" placeholder="0.00" bind:value={max} />
		</label>
		<label>
			{t('ruleform.category')}
			<select bind:value={category}>
				<option value="">{t('ruleform.no_category')}</option>
				{#each categories as option (option.id)}
					<option value={option.id}>{option.name}</option>
				{/each}
			</select>
		</label>
		<label>
			{t('ruleform.payee')}
			<select bind:value={payee}>
				<option value="">{t('ruleform.no_payee')}</option>
				<option value="text">{t('ruleform.payee_from_text')}</option>
				{#each payees as option (option.id)}
					<option value={option.id}>{option.name}</option>
				{/each}
			</select>
		</label>
	</div>
	{#if error}<p class="alert" role="alert">{error}</p>{/if}
	{#if done}<p class="notice" role="status">{done}</p>{/if}
	<div><button type="submit" disabled={busy}>{t('ruleform.add')}</button></div>
</form>

<style>
	.fields {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 220px), 1fr));
		gap: 12px;
		align-items: end;
	}
</style>
