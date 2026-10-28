<script lang="ts">
	// «Payments owed to you»: each employer's report at today's date.
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { day, today } from '#lib/format.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import Employer from './Employer.svelte';

	let payroll = $state<Schemas['Payroll'] | null>(null);
	let error = $state('');

	async function load() {
		error = '';
		const {
			data,
			error: failure,
			response
		} = await api.GET('/api/v1/payroll', { params: { query: { as_of: today() } } });
		if (!data) {
			error = problem(failure, response?.status, t('payroll.failed'));
			return;
		}
		payroll = data;
	}

	onMount(load);
</script>

{#if error || payroll?.employers.length}
	<section class="card" aria-labelledby="payroll-title">
		<h2 id="payroll-title">{t('payroll.title')}</h2>
		{#if error}
			<p class="alert" role="alert">
				{error} <button class="secondary" onclick={load}>{t('app.try_again')}</button>
			</p>
		{:else if payroll}
			<p class="hint">{t('payroll.as_of', { date: day(payroll.as_of) })}</p>
			{#each payroll.employers as employer (employer.employer_id)}
				<Employer {employer} />
			{/each}
		{/if}
	</section>
{/if}
