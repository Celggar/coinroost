<script lang="ts">
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import CategoryForm from '#lib/rules/CategoryForm.svelte';
	import Changes from '#lib/rules/Changes.svelte';
	import RuleForm from '#lib/rules/RuleForm.svelte';
	import { actions, conditions, names as nameMaps } from '#lib/rules/describe.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import Confirm from '#lib/ui/Confirm.svelte';
	import { notify } from '#lib/ui/notices.svelte.ts';
	import { glide, pop, rise, vanish } from '#lib/motion.ts';

	let rules = $state<Schemas['Rule'][]>([]);
	let categories = $state<Schemas['Category'][]>([]);
	let payees = $state<Schemas['Payee'][]>([]);
	let accounts = $state<Schemas['Account'][]>([]);
	let loadError = $state('');
	let loaded = $state(false);
	let busy = $state(false);
	let title = $state<HTMLHeadingElement>();

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
		const { error, response } = await api.PATCH('/api/v1/rules/{rule_id}', {
			params: { path: { rule_id: rule.id } },
			body: { position: to }
		});
		busy = false;
		if (!response.ok) {
			notify(problem(error, response.status, t('rules.move_failed')), 'bad');
		}
		await load();
	}

	async function remove(rule: Schemas['Rule']) {
		busy = true;
		const { error, response } = await api.DELETE('/api/v1/rules/{rule_id}', {
			params: { path: { rule_id: rule.id } }
		});
		busy = false;
		if (!response.ok) {
			notify(problem(error, response.status, t('rules.delete_failed')), 'bad');
		}
		await load();
		// The rule and its buttons are gone: the focus goes to the page's title, not to the page's start.
		if (response.ok) title?.focus();
	}

	onMount(load);
</script>

<svelte:head>
	<title>{t('app.title', { page: t('rules.title') })}</title>
</svelte:head>

<h1 tabindex="-1" bind:this={title}>{t('rules.title')}</h1>
<p class="hint">{t('rules.hint')}</p>

{#if loadError}
	<p class="alert" role="alert">
		{loadError} <button class="secondary" onclick={load}>{t('app.try_again')}</button>
	</p>
{:else if loaded}
	<div class="stack">
		<div class="swap">
			{#if rules.length}
				<ol class="rules" aria-label={t('rules.list')} in:rise out:vanish>
					{#each rules as rule, index (rule.id)}
						<li class="card rule" in:rise out:vanish animate:glide>
							<div class="head">
								<span class="position" aria-hidden="true">{rule.position}</span>
								<h2>{rule.name}</h2>
								{#if !rule.enabled}<span class="chip none">{t('rules.off')}</span>{/if}
							</div>
							<p><strong>{t('rules.when')}</strong> {conditions(rule, names).join(' · ')}</p>
							<p><strong>{t('rules.sets')}</strong> {actions(rule, names).join(' · ')}</p>
							<div class="buttons">
								<!-- The first rule cannot go up and the last cannot go down: those buttons are not shown. -->
								{#if index > 0}
									<button
										class="secondary"
										onclick={() => move(rule, rule.position - 1)}
										disabled={busy}
										aria-label={t('rules.up_named', { name: rule.name })}
										in:pop={{ start: 0.9 }}
										out:vanish>{t('rules.up')}</button
									>
								{/if}
								{#if index < rules.length - 1}
									<button
										class="secondary"
										onclick={() => move(rule, rule.position + 1)}
										disabled={busy}
										aria-label={t('rules.down_named', { name: rule.name })}
										in:pop={{ start: 0.9 }}
										out:vanish>{t('rules.down')}</button
									>
								{/if}
								<Confirm
									label={t('rules.delete')}
									named={t('rules.delete_named', { name: rule.name })}
									yes={t('rules.delete_it')}
									no={t('rules.keep_it')}
									{busy}
									onconfirm={() => remove(rule)}
								/>
							</div>
						</li>
					{/each}
				</ol>
			{:else}
				<p in:rise out:vanish>{t('rules.none_yet')}</p>
			{/if}
		</div>

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
		gap: var(--space-md);
	}
	.rule p {
		margin: 0;
		overflow-wrap: anywhere;
	}
	.head {
		display: flex;
		align-items: center;
		gap: var(--space-md);
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
		align-items: center;
		gap: var(--space-sm);
	}
</style>
