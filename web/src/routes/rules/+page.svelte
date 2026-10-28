<script lang="ts">
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import CategoryForm from '#lib/rules/CategoryForm.svelte';
	import Changes from '#lib/rules/Changes.svelte';
	import RuleForm from '#lib/rules/RuleForm.svelte';
	import { actions, conditions, names as nameMaps } from '#lib/rules/describe.ts';

	let rules = $state<Schemas['Rule'][]>([]);
	let categories = $state<Schemas['Category'][]>([]);
	let payees = $state<Schemas['Payee'][]>([]);
	let accounts = $state<Schemas['Account'][]>([]);
	let loadError = $state('');
	let actionError = $state('');
	let loaded = $state(false);
	let busy = $state(false);
	let confirming = $state<string | null>(null);

	const names = $derived(nameMaps(categories, payees, accounts));

	async function load() {
		loadError = '';
		const [r, c, p, a] = await Promise.all([
			api.GET('/api/v1/rules'),
			api.GET('/api/v1/categories'),
			api.GET('/api/v1/payees'),
			api.GET('/api/v1/accounts')
		]);
		const failed = [r, c, p, a].find((x) => !x.data);
		if (failed || !r.data || !c.data || !p.data || !a.data) {
			loadError = problem(failed?.error, failed?.response?.status, 'The rules could not be loaded');
			return;
		}
		rules = r.data.items;
		categories = c.data.items;
		payees = p.data.items;
		accounts = a.data.items;
		loaded = true;
	}

	async function move(rule: Schemas['Rule'], to: number) {
		busy = true;
		actionError = '';
		const { error, response } = await api.PATCH('/api/v1/rules/{rule_id}', {
			params: { path: { rule_id: rule.id } },
			body: { position: to }
		});
		busy = false;
		if (!response.ok) {
			actionError = problem(error, response.status, 'The rule could not be moved');
		}
		await load();
	}

	async function remove(rule: Schemas['Rule']) {
		busy = true;
		actionError = '';
		const { error, response } = await api.DELETE('/api/v1/rules/{rule_id}', {
			params: { path: { rule_id: rule.id } }
		});
		busy = false;
		confirming = null;
		if (!response.ok) {
			actionError = problem(error, response.status, 'The rule could not be deleted');
		}
		await load();
	}

	onMount(load);
</script>

<svelte:head>
	<title>Rules · Coinroost</title>
</svelte:head>

<h1>Rules</h1>
<p class="hint">
	The first rule, from the top, that matches a transaction sets its category and payee; the rules
	below it are not looked at.
</p>

{#if loadError}
	<p class="alert" role="alert">
		{loadError} <button class="secondary" onclick={load}>Try again</button>
	</p>
{:else if loaded}
	<div class="stack">
		{#if actionError}<p class="alert" role="alert">{actionError}</p>{/if}
		{#if rules.length}
			<ol class="rules" aria-label="Rules in order">
				{#each rules as rule, index (rule.id)}
					<li class="card rule">
						<div class="head">
							<span class="position" aria-hidden="true">{rule.position}</span>
							<h2>{rule.name}</h2>
							{#if !rule.enabled}<span class="chip none">Off</span>{/if}
						</div>
						<p><strong>When</strong> {conditions(rule, names).join(' · ')}</p>
						<p><strong>Sets</strong> {actions(rule, names).join(' · ')}</p>
						<div class="buttons">
							<button
								class="secondary"
								onclick={() => move(rule, rule.position - 1)}
								disabled={busy || index === 0}
								aria-label={`Move ${rule.name} up`}>Up</button
							>
							<button
								class="secondary"
								onclick={() => move(rule, rule.position + 1)}
								disabled={busy || index === rules.length - 1}
								aria-label={`Move ${rule.name} down`}>Down</button
							>
							{#if confirming === rule.id}
								<button onclick={() => remove(rule)} disabled={busy}>Delete it</button>
								<button class="secondary" onclick={() => (confirming = null)}>Keep it</button>
							{:else}
								<button
									class="secondary"
									onclick={() => (confirming = rule.id)}
									aria-label={`Delete ${rule.name}`}>Delete</button
								>
							{/if}
						</div>
					</li>
				{/each}
			</ol>
		{:else}
			<p>No rules yet.</p>
		{/if}

		<RuleForm {categories} {payees} {accounts} oncreated={load} />
		<CategoryForm oncreated={load} />
		<Changes {names} onapplied={load} />
	</div>
{:else}
	<p class="hint">Loading…</p>
{/if}

<style>
	.rules {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 8px;
	}
	.rule p {
		margin: 0;
		overflow-wrap: anywhere;
	}
	.head {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.position {
		display: inline-grid;
		place-items: center;
		min-width: 32px;
		height: 32px;
		border-radius: 999px;
		background: var(--bg);
		font-weight: 700;
	}
	.buttons {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}
</style>
