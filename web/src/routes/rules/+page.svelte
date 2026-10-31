<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { prefersReducedMotion } from 'svelte/motion';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import CategoryForm from '#lib/rules/CategoryForm.svelte';
	import Changes from '#lib/rules/Changes.svelte';
	import Tick from '#lib/ui/Tick.svelte';
	import RuleForm from '#lib/rules/RuleForm.svelte';
	import { actions, conditions, names as nameMaps } from '#lib/rules/describe.ts';
	import { LIST } from '#lib/format.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import Confirm from '#lib/ui/Confirm.svelte';
	import { focusOn, retry } from '#lib/ui/focus.ts';
	import { focusShown, keepWhole } from '#lib/ui/form.ts';
	import { notify } from '#lib/ui/notices.svelte.ts';
	import { arrive, glide, placeholder, pop, rise, vanish } from '#lib/motion.ts';

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
		const way = to < rule.position ? 'up' : 'down';
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
		// The buttons were disabled while it moved, which drops the focus: it follows the rule, to the same button,
		// or to its other one when the rule reached the top or the bottom (the one leaving is inert, so skipped).
		await tick();
		if (!focusOn(`[data-move="${rule.id}-${way}"]`, `[data-move^="${rule.id}-"]`)) return;
		// The rule glides to its place after the focus went to it: once it lands, the page follows it, or near the
		// top of the screen the focus ended above it (pass 7).
		const row = (document.activeElement as HTMLElement).closest('li');
		await new Promise(requestAnimationFrame);
		await Promise.all(row?.getAnimations().map((a) => a.finished.catch(() => undefined)) ?? []);
		row?.scrollIntoView({
			block: 'nearest',
			behavior: prefersReducedMotion.current ? 'auto' : 'smooth'
		});
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

	// A new rule rises into the list above the form while the page stands still, so the form moves down and the
	// rule's card can stand cut at the screen's top: once it lands, the page settles where nothing is cut (pass 9).
	async function added() {
		await load();
		await tick();
		const list = document.querySelector('.rules');
		await Promise.all(
			list?.getAnimations({ subtree: true }).map((a) => a.finished.catch(() => {})) ?? []
		);
		await keepWhole();
	}

	// A link to the category form (from an uncategorized queue with no category to choose) lands in its name field:
	// the form only exists once the page has loaded, too late for the browser's own jump to the anchor.
	onMount(async () => {
		await load();
		if (location.hash === '#new-category') {
			await tick();
			const form = document.getElementById('new-category');
			const name = form?.querySelector('input');
			// The form from its title, its name field focused and its button in view.
			if (form && name) focusShown(name, form);
		}
	});
</script>

<svelte:head>
	<title>{t('app.title', { page: t('rules.title') })}</title>
</svelte:head>

<h1 tabindex="-1" bind:this={title}>{t('rules.title')}</h1>
<p class="hint">{t('rules.hint')}</p>

{#if loadError}
	<p class="alert" role="alert" out:placeholder>
		{loadError}
		<button class="secondary" onclick={(event) => retry(event.currentTarget, load)}
			>{t('app.try_again')}</button
		>
	</p>
{:else if loaded}
	<div class="stack" in:arrive>
		<div class="in-place">
			{#if rules.length}
				<ol class="rules" aria-label={t('rules.list')} in:rise out:vanish>
					{#each rules as rule, index (rule.id)}
						<li class="card rule" in:rise out:vanish animate:glide>
							<div class="head">
								<span class="position" aria-hidden="true"
									><Tick value={rule.position}>{rule.position}</Tick></span
								>
								<h2>{rule.name}</h2>
								{#if !rule.enabled}<span class="chip none">{t('rules.off')}</span>{/if}
							</div>
							<p><strong>{t('rules.when')}</strong> {conditions(rule, names).join(LIST)}</p>
							<p><strong>{t('rules.sets')}</strong> {actions(rule, names).join(LIST)}</p>
							<div class="buttons">
								<!-- The first rule cannot go up and the last cannot go down: those buttons are not shown. -->
								{#if index > 0}
									<button
										class="secondary"
										data-move="{rule.id}-up"
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
										data-move="{rule.id}-down"
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

		<RuleForm {categories} {payees} {accounts} oncreated={added} />
		<CategoryForm oncreated={load} />
		<Changes {names} onapplied={load} />
	</div>
{:else}
	<p class="hint loading" out:placeholder>{t('app.loading')}</p>
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
	/* Number, name and chip share the name's first baseline, so a long name wraps under itself, not around the number. */
	.head {
		display: flex;
		align-items: baseline;
		gap: var(--space-md);
	}
	.position {
		flex: none;
		display: inline-grid;
		place-items: center;
		min-width: 32px;
		height: 32px;
		border-radius: 999px;
		/* The accent tint of the rail's current page: the page's own background was a hole in the card (pass 3). */
		background: color-mix(in oklab, var(--accent) 14%, var(--surface));
		font-weight: 700;
	}
	/* Forced colors drop the tint: a border keeps the number in its disc. */
	@media (forced-colors: active) {
		.position {
			border: 1px solid CanvasText;
		}
	}
	.buttons {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-sm);
	}
</style>
