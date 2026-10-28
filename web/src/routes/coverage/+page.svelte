<script lang="ts">
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { span, today } from '#lib/format.ts';

	let coverage = $state<Schemas['Coverage'] | null>(null);
	let error = $state('');
	let busy = $state(false);

	const delays: Record<Schemas['Delay'], { label: string; tone: string }> = {
		late: { label: 'Late', tone: 'bad' },
		warning: { label: 'Check', tone: 'warn' },
		no_data: { label: 'No data', tone: 'none' },
		up_to_date: { label: 'Up to date', tone: 'ok' }
	};

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
			error = problem(failure, response?.status, 'Coverage could not be loaded');
			return;
		}
		coverage = data;
	}

	onMount(load);
</script>

<svelte:head>
	<title>Coverage · Coinroost</title>
</svelte:head>

<div class="title">
	<h1>Coverage</h1>
	<button class="secondary" onclick={load} disabled={busy}
		>{busy ? 'Refreshing…' : 'Refresh'}</button
	>
</div>
<p class="hint">
	Up to when each account has data, and what the imported files cover. Status <strong>Check</strong
	>: more than 15 days without data; <strong>Late</strong>: more than 40. A statement cycle is about
	30 days, but a quiet account can be still on purpose.
</p>

{#if error}
	<p class="alert" role="alert">{error}</p>
{:else if coverage}
	{#if coverage.items.length === 0}
		<p>There are no accounts yet. <a href="/accounts">Create one</a>.</p>
	{:else}
		<div class="table-wrap">
			<table>
				<thead>
					<tr>
						<th>Account</th>
						<th>Status</th>
						<th>Latest</th>
						<th class="number">Days since</th>
						<th>Imported</th>
						<th>Gaps</th>
					</tr>
				</thead>
				<tbody>
					{#each coverage.items as item (item.account_id)}
						<tr>
							<td>{item.name}</td>
							<td>
								<span class="chip {delays[item.delay].tone}">{delays[item.delay].label}</span>
							</td>
							<td>{item.latest ?? '—'}</td>
							<td class="number">{item.days_since ?? '—'}</td>
							<td>
								{#each item.ranges as range (range.from)}
									<div>{span(range.from, range.to)}</div>
								{:else}
									<span class="hint">No imports</span>
								{/each}
							</td>
							<td>
								{#each item.gaps as gap (gap.from)}
									<div>
										<span class="chip {delays[gap.delay].tone}">{gap.days} days</span>
										{span(gap.from, gap.to)}
									</div>
								{:else}
									<span class="hint">None</span>
								{/each}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
{:else}
	<p class="hint">Loading…</p>
{/if}

<style>
	.title {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 12px;
	}
	td div + div {
		margin-top: 4px;
	}
</style>
