<script lang="ts">
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import CategoryForm from '#lib/rules/CategoryForm.svelte';
	import Changes from '#lib/rules/Changes.svelte';
	import RuleForm from '#lib/rules/RuleForm.svelte';
	import { actions, conditions, names as nameMaps } from '#lib/rules/describe.ts';
	import { t } from '#lib/i18n.svelte.ts';

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
			loadError = problem(failed?.error, failed?.response?.status, t('rules.failed'));
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
			actionError = problem(error, response.status, t('rules.move_failed'));
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
			actionError = problem(error, response.status, t('rules.delete_failed'));
		}
		await load();
	}

	onMount(load);
</script>

<svelte:head>
	<title>{t('app.title', { page: t('rules.title') })}</title>
</svelte:head>

<h1>{t('rules.title')}</h1>
<p class="hint">{t('rules.hint')}</p>

{#if loadError}
	<p class="alert" role="alert">
		{loadError} <button class="secondary" onclick={load}>{t('app.try_again')}</button>
	</p>
{:else if loaded}
	<div class="stack">
		{#if actionError}<p class="alert" role="alert">{actionError}</p>{/if}
		{#if rules.length}
			<ol class="rules" aria-label={t('rules.list')}>
				{#each rules as rule, index (rule.id)}
					<li class="card rule">
						<div class="head">
							<span class="position" aria-hidden="true">{rule.position}</span>
							<h2>{rule.name}</h2>
							{#if !rule.enabled}<span class="chip none">{t('rules.off')}</span>{/if}
						</div>
						<p><strong>{t('rules.when')}</strong> {conditions(rule, names).join(' · ')}</p>
						<p><strong>{t('rules.sets')}</strong> {actions(rule, names).join(' · ')}</p>
						<div class="buttons">
							<button
								class="secondary"
								onclick={() => move(rule, rule.position - 1)}
								disabled={busy || index === 0}
								aria-label={t('rules.up_named', { name: rule.name })}>{t('rules.up')}</button
							>
							<button
								class="secondary"
								onclick={() => move(rule, rule.position + 1)}
								disabled={busy || index === rules.length - 1}
								aria-label={t('rules.down_named', { name: rule.name })}>{t('rules.down')}</button
							>
							{#if confirming === rule.id}
								<button onclick={() => remove(rule)} disabled={busy}>{t('rules.delete_it')}</button>
								<button class="secondary" onclick={() => (confirming = null)}
									>{t('rules.keep_it')}</button
								>
							{:else}
								<button
									class="secondary"
									onclick={() => (confirming = rule.id)}
									aria-label={t('rules.delete_named', { name: rule.name })}
									>{t('rules.delete')}</button
								>
							{/if}
						</div>
					</li>
				{/each}
			</ol>
		{:else}
			<p>{t('rules.none_yet')}</p>
		{/if}

		<RuleForm {categories} {payees} {accounts} oncreated={load} />
		<CategoryForm oncreated={load} />
		<Changes {names} onapplied={load} />
	</div>
{:else}
	<p class="hint">{t('app.loading')}</p>
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
