<script lang="ts">
	import { scrollable } from '#lib/ui/scrollable.ts';
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { day, span, today } from '#lib/format.ts';
	import { plural, t, type Key } from '#lib/i18n.svelte.ts';
	import Rich from '#lib/Rich.svelte';

	let coverage = $state<Schemas['Coverage'] | null>(null);
	let error = $state('');
	let busy = $state(false);

	const tones: Record<Schemas['Delay'], string> = {
		late: 'bad',
		warning: 'warn',
		no_data: 'none',
		up_to_date: 'ok'
	};
	const delayKeys = {
		late: 'delay.late',
		warning: 'delay.warning',
		no_data: 'delay.no_data',
		up_to_date: 'delay.up_to_date'
	} satisfies Record<Schemas['Delay'], Key>;
	const delayName = (delay: Schemas['Delay']) => t(delayKeys[delay]);

	async function load() {
		busy = true;
		error = '';
		const {
			data,
			error: failure,
			response
		} = await api.GET('/api/v1/coverage', {
			params: { query: { today: today() } }
		});
		busy = false;
		if (!data) {
			// Never an empty table: it would read as «everything is up to date».
			coverage = null;
			error = problem(failure, response?.status, t('coverage.failed'));
			return;
		}
		coverage = data;
	}

	onMount(load);
</script>

<svelte:head>
	<title>{t('app.title', { page: t('coverage.title') })}</title>
</svelte:head>

<div class="title">
	<h1>{t('coverage.title')}</h1>
	<button class="secondary" onclick={load} disabled={busy}
		>{busy ? t('coverage.refreshing') : t('coverage.refresh')}</button
	>
</div>
<p class="hint">
	<Rich text={t('coverage.hint', { check: t('delay.warning'), late: t('delay.late') })} />
</p>

{#if error}
	<p class="alert" role="alert">{error}</p>
{:else if coverage}
	{#if coverage.items.length === 0}
		<p>{t('coverage.no_accounts')} <a href="/accounts">{t('coverage.create_one')}</a></p>
	{:else}
		<div class="table-wrap" use:scrollable={t('coverage.title')}>
			<table class="rows-on-phone">
				<thead>
					<tr>
						<th>{t('column.account')}</th>
						<th>{t('column.status')}</th>
						<th>{t('coverage.latest')}</th>
						<th class="number">{t('coverage.days_since')}</th>
						<th>{t('coverage.imported')}</th>
						<th>{t('coverage.gaps')}</th>
					</tr>
				</thead>
				<tbody>
					{#each coverage.items as item (item.account_id)}
						<tr>
							<td data-label={t('column.account')}>{item.name}</td>
							<td data-label={t('column.status')}>
								<span class="chip {tones[item.delay]}">{delayName(item.delay)}</span>
							</td>
							<td data-label={t('coverage.latest')} class="date"
								>{item.latest ? day(item.latest) : '—'}</td
							>
							<td data-label={t('coverage.days_since')} class="number">{item.days_since ?? '—'}</td>
							<td data-label={t('coverage.imported')}>
								{#each item.ranges as range (range.from)}
									<div>{span(range.from, range.to)}</div>
								{:else}
									<span class="hint">{t('coverage.no_imports')}</span>
								{/each}
							</td>
							<td data-label={t('coverage.gaps')}>
								{#each item.gaps as gap (gap.from)}
									<div>
										<span class="chip {tones[gap.delay]}"
											>{plural(gap.days, 'coverage.days_one', 'coverage.days_other')}</span
										>
										{span(gap.from, gap.to)}
									</div>
								{:else}
									<span class="hint">{t('coverage.no_gaps')}</span>
								{/each}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
{:else}
	<p class="hint">{t('app.loading')}</p>
{/if}

<style>
	/* The title's margins move to the row, so «Actualizar» is centred on the title's text, not on its margins. */
	.title {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--space-md);
		margin: var(--space-sm) 0 var(--space-lg);
	}
	.title h1 {
		margin: 0;
	}
	/* A phone shows the title in its top bar: the button keeps its place at the end of the row, beside nothing. */
	@media (max-width: 899px) {
		.title {
			justify-content: flex-end;
			margin-bottom: var(--space-md);
		}
	}
	.date {
		white-space: nowrap;
	}
	td div + div {
		margin-top: var(--space-xs);
	}
</style>
