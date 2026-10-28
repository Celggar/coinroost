<script lang="ts">
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { toMinor } from '#lib/rules/describe.ts';

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
			error = 'Amounts are numbers with up to two decimals, such as 25 or 25.50.';
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
			error = problem(failure, response?.status, 'The rule could not be created');
			return;
		}
		done = `Added «${data.name}» at position ${data.position}.`;
		name = text = min = max = category = payee = account = '';
		direction = '';
		oncreated(data);
	}
</script>

<form class="card" onsubmit={submit} oninput={clear} aria-label="New rule">
	<h2>New rule</h2>
	<p class="hint">
		It goes last. It sorts files imported from now on; existing transactions change only when you
		check and apply below.
	</p>
	<div class="fields">
		<label>
			Rule name
			<input required maxlength="200" bind:value={name} />
		</label>
		<label>
			Description has
			<span class="hint">Upper or lower case alike; accents and spaces count.</span>
			<input maxlength="200" bind:value={text} />
		</label>
		<label>
			Money
			<select bind:value={direction}>
				<option value="">In or out</option>
				<option value="out">Out (spending)</option>
				<option value="in">In (income)</option>
			</select>
		</label>
		<label>
			Account
			<select bind:value={account}>
				<option value="">Any account</option>
				{#each accounts as option (option.id)}
					<option value={option.id}>{option.name}</option>
				{/each}
			</select>
		</label>
		<label>
			Amount from
			<input inputmode="decimal" placeholder="0.00" bind:value={min} />
		</label>
		<label>
			Amount up to
			<input inputmode="decimal" placeholder="0.00" bind:value={max} />
		</label>
		<label>
			Sets category
			<select bind:value={category}>
				<option value="">No category</option>
				{#each categories as option (option.id)}
					<option value={option.id}>{option.name}</option>
				{/each}
			</select>
		</label>
		<label>
			Sets payee
			<select bind:value={payee}>
				<option value="">No payee</option>
				<option value="text">The name after the text (a payment channel)</option>
				{#each payees as option (option.id)}
					<option value={option.id}>{option.name}</option>
				{/each}
			</select>
		</label>
	</div>
	{#if error}<p class="alert" role="alert">{error}</p>{/if}
	{#if done}<p class="notice" role="status">{done}</p>{/if}
	<div><button type="submit" disabled={busy}>Add rule</button></div>
</form>

<style>
	.fields {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 220px), 1fr));
		gap: 12px;
		align-items: end;
	}
</style>
