<script lang="ts">
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { money } from '#lib/format.ts';
	import { accountName, passLabel, type AccountNames } from '#lib/transfers/legs.ts';

	let {
		preview,
		names,
		onchanged
	}: { preview: Schemas['TransferPreview']; names: AccountNames; onchanged: () => void } = $props();

	let error = $state('');
	let done = $state('');
	let busy = $state(false);

	async function apply() {
		busy = true;
		error = '';
		done = '';
		const {
			data,
			error: failure,
			response
		} = await api.POST('/api/v1/transfers/apply', {
			params: { header: { 'If-Match': preview.tag } }
		});
		busy = false;
		if (!data) {
			error =
				response?.status === 412
					? 'The pairs changed since this page loaded: they are shown again, check them.'
					: problem(failure, response?.status, 'The pairs could not be made');
		} else {
			done = `Paired ${data.paired}.`;
		}
		onchanged();
	}
</script>

<section class="card" aria-labelledby="waiting-title">
	<h2 id="waiting-title">Pairs waiting</h2>
	{#if error}<p class="alert" role="alert">{error}</p>{/if}
	{#if done}<p class="notice" role="status">{done}</p>{/if}
	{#if preview.pairs.length === 0}
		<p>Nothing waiting: imports pair what is certain on their own.</p>
	{:else}
		<p class="hint">Pairing changes no balance: the same two legs become one transfer.</p>
		<ul class="pairs" aria-label="Pairs waiting">
			{#each preview.pairs as pair (pair.outflow.transaction_id)}
				<li>
					<div class="line">
						<strong>{money(-pair.outflow.amount_minor, pair.outflow.currency)}</strong>
						<span>
							{accountName(pair.outflow.account_id, names)} ({pair.outflow.date}) → {accountName(
								pair.inflow.account_id,
								names
							)} ({pair.inflow.date})
						</span>
					</div>
					<div class="hint">{pair.outflow.description} · found because {passLabel(pair.pass)}</div>
					{#if pair.drops_manual}
						<span class="chip warn">Drops a category you chose for the incoming side</span>
					{/if}
				</li>
			{/each}
		</ul>
		<div>
			<button onclick={apply} disabled={busy}>Pair {preview.pairs.length}</button>
		</div>
	{/if}
</section>

<style>
	.pairs {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 8px;
	}
	.pairs li {
		border-top: 1px solid var(--border);
		padding-top: 8px;
		display: grid;
		gap: 4px;
		justify-items: start;
	}
	.line {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 12px;
		overflow-wrap: anywhere;
	}
	strong {
		font-variant-numeric: tabular-nums;
	}
</style>
