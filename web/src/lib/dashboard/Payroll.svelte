<script lang="ts">
	// «Payments owed to you»: each employer's report at today's date. The page holds the answer (bound), so «Por
	// revisar» can say who owes without asking again.
	import { retry } from '#lib/ui/focus.ts';
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { today, wholeDay } from '#lib/format.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import Employer from './Employer.svelte';
	import { arrive, placeholder } from '#lib/motion.ts';

	let {
		payroll = $bindable(null),
		settled = $bindable(false),
		ready
	}: { payroll?: Schemas['Payroll'] | null; settled?: boolean; ready: boolean } = $props();
	let error = $state('');

	// A failed load keeps its alert until the next answer: cleared at the start, it took the whole card away, and the
	// focus of «Intentar de nuevo» with it (pass 10).
	async function load() {
		const {
			data,
			error: failure,
			response
		} = await api.GET('/api/v1/payroll', { params: { query: { as_of: today() } } });
		// Answered, either way: the summary waits for it so «Por revisar» does not grow a row after it shows.
		settled = true;
		if (!data) {
			error = problem(failure, response?.status, t('payroll.failed'));
			return;
		}
		error = '';
		payroll = data;
	}

	onMount(load);
</script>

<!-- Asked at once, shown with the page's top (`ready`): alone under «Cargando…», it fell 1200 px in one frame as the top
     came (pass 36). -->
{#if ready && (error || payroll?.employers.length)}
	<section class="card" id="payroll" aria-labelledby="payroll-title" in:arrive>
		<!-- `tabindex`: «Ver» on the summary's card lands here, and the browser gives the focus only to what can hold it. -->
		<h2 id="payroll-title" tabindex="-1">{t('payroll.title')}</h2>
		{#if error}
			<p class="alert" role="alert" in:arrive out:placeholder>
				{error}
				<button
					class="secondary"
					onclick={(event) => retry(event.currentTarget, load, '#payroll-title')}
					>{t('app.try_again')}</button
				>
			</p>
		{:else if payroll}
			<div class="stack" in:arrive>
				<p class="hint">{t('payroll.as_of', { date: wholeDay(payroll.as_of) })}</p>
				{#each payroll.employers as employer (employer.employer_id)}
					<Employer {employer} />
				{/each}
			</div>
		{/if}
	</section>
{/if}
