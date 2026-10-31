<script lang="ts">
	import { scrollable } from '#lib/ui/scrollable.ts';
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { day, span, today } from '#lib/format.ts';
	import { plural, t, type Key } from '#lib/i18n.svelte.ts';
	import Rich from '#lib/Rich.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import { notify } from '#lib/ui/notices.svelte.ts';

	let coverage = $state<Schemas['Coverage'] | null>(null);
	let error = $state('');
	let busy = $state(false);
	// The «Refreshing…» label only once a refresh takes a while: a fast one would flash it for a frame or two.
	let slow = $state(false);

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

	async function load(asked = false) {
		busy = true;
		error = '';
		const wait = setTimeout(() => (slow = true), 300);
		const {
			data,
			error: failure,
			response
		} = await api.GET('/api/v1/coverage', {
			params: { query: { today: today() } }
		});
		clearTimeout(wait);
		busy = false;
		slow = false;
		if (!data) {
			// Never an empty table: it would read as «everything is up to date».
			coverage = null;
			error = problem(failure, response?.status, t('coverage.failed'));
			return;
		}
		coverage = data;
		// A refresh the person asked for says it happened: the table often looks the same afterwards.
		if (asked) notify(t('coverage.refreshed'));
	}

	onMount(load);
</script>

<svelte:head>
	<title>{t('app.title', { page: t('coverage.title') })}</title>
</svelte:head>

<div class="head">
	<h1>{t('coverage.title')}</h1>
	<p class="hint">
		<Rich text={t('coverage.hint', { check: t('delay.warning'), late: t('delay.late') })} />
	</p>
	<!-- aria-disabled, not disabled: a disabled button drops the focus, and the person pressed this one. Both labels
	     share one cell, so the button keeps its width while one fades into the other; the hidden one is not read. -->
	<!-- Nothing to refresh before there is an account: the empty card below says what to do instead. -->
	{#if !(coverage && coverage.items.length === 0)}
		<button class="secondary refresh" onclick={() => busy || load(true)} aria-disabled={busy}>
			<span class:shown={!slow} aria-hidden={slow}>{t('coverage.refresh')}</span>
			<span class:shown={slow} aria-hidden={!slow}>{t('coverage.refreshing')}</span>
		</button>
	{/if}
</div>

{#if error}
	<p class="alert" role="alert">{error}</p>
{:else if coverage}
	{#if coverage.items.length === 0}
		<EmptyState
			icon="bank"
			text={t('coverage.no_accounts')}
			href="/accounts"
			action={t('coverage.create_one')}
		/>
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
							<td data-label={t('coverage.imported')} class="ranges">
								{#each item.ranges as range (range.from)}
									<div>{span(range.from, range.to)}</div>
								{:else}
									<span class="hint">{t('coverage.no_imports')}</span>
								{/each}
							</td>
							<td data-label={t('coverage.gaps')} class="ranges">
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
	<p class="hint loading">{t('app.loading')}</p>
{/if}

<style>
	/* The title's margins move to the grid, so «Actualizar» is centred on the title's text, not on its margins; the
	   explanation runs under both. */
	.head {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		grid-template-areas: 'title action' 'hint hint';
		align-items: center;
		gap: var(--space-lg) var(--space-md);
		margin: var(--space-sm) 0 var(--space-lg);
	}
	.head h1 {
		grid-area: title;
		margin: 0;
	}
	.head .hint {
		grid-area: hint;
	}
	/* A phone shows the title in its top bar: alone at the row's end the button read as a leftover, so it follows the
	   explanation instead, where the eye arrives after reading it. */
	@media (max-width: 899px) {
		.head {
			grid-template-columns: minmax(0, 1fr);
			grid-template-areas: 'hint' 'action';
			row-gap: var(--space-md);
		}
		.head h1 {
			grid-area: auto;
		}
		.refresh {
			justify-self: start;
		}
	}
	.refresh {
		grid-area: action;
		display: inline-grid;
		place-items: center;
	}
	/* Beside the title the button is 14 px taller than it (44 against 30): negative margins let the row take the
	   title's height, so the title sits where it sits on the sibling tabs and does not jump when they switch. */
	@media (min-width: 900px) {
		.refresh {
			margin-block: -7px;
		}
	}
	.refresh > span {
		grid-area: 1 / 1;
		opacity: 0;
		transition: opacity var(--duration-short) var(--ease-standard);
	}
	.refresh > .shown {
		opacity: 1;
	}
	.date {
		white-space: nowrap;
	}
	/* A range stays on one line once the account's name keeps room beside it: from 1100 px, with the rail, the name
	   column is 184 px wide. Below that a range breaks at its dash (each date holds together), and from the rail's
	   900 px the cells are tighter: at 900 px the six columns were 97 px wider than the card. */
	@media (min-width: 1100px) {
		.ranges div {
			white-space: nowrap;
		}
	}
	@media (min-width: 900px) and (max-width: 1099px) {
		th,
		td {
			padding-inline: var(--space-sm);
		}
	}
	td div + div {
		margin-top: var(--space-xs);
	}
</style>
