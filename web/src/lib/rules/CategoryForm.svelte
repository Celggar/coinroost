<script lang="ts">
	import { api, problem, type Schemas } from '#lib/api/client.ts';

	let { oncreated }: { oncreated: (category: Schemas['Category']) => void } = $props();

	let name = $state('');
	let kind = $state<Schemas['CategoryKind']>('expense');
	let error = $state('');
	let done = $state('');
	let busy = $state(false);

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
			error = problem(failure, response?.status, 'The category could not be created');
			return;
		}
		done = `Created ${data.name}.`;
		name = '';
		oncreated(data);
	}
</script>

<form class="card" onsubmit={submit} aria-label="New category">
	<h2>New category</h2>
	<div class="fields">
		<label>
			Category name
			<input required maxlength="100" bind:value={name} />
		</label>
		<label>
			Holds
			<select bind:value={kind}>
				<option value="expense">Spending</option>
				<option value="income">Income</option>
				<option value="transfer">Moves between my accounts</option>
			</select>
		</label>
	</div>
	{#if error}<p class="alert" role="alert">{error}</p>{/if}
	{#if done}<p class="notice" role="status">{done}</p>{/if}
	<div><button type="submit" disabled={busy}>Create category</button></div>
</form>

<style>
	.fields {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 220px), 1fr));
		gap: 12px;
	}
</style>
