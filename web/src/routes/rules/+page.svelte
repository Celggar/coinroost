<script lang="ts">
	import { onMount, tick } from 'svelte';
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
	import { focusShown, showWhole } from '#lib/ui/landing.ts';
	import { notify } from '#lib/ui/notices.svelte.ts';
	import { arrive, glide, grow, placeholder, pop, swapIn, vanish } from '#lib/motion.ts';

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
		const moved = focusOn(`[data-move="${rule.id}-${way}"]`, `[data-move^="${rule.id}-"]`);
		// The rule glides to its place after the focus went to it: once it lands, the page shows it whole, or near the
		// top of the screen the focus ended above it (pass 7).
		const row = moved?.closest('li');
		if (row) await showWhole(row);
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
		// The rule and its buttons are gone: the focus goes to the page's title. Where the title shows (from 900 px) the
		// page glides to it; on a phone the top bar holds the title and the page stays. Focused bare, the page jumped to
		// its start in one frame (pass 15).
		if (response.ok && title) {
			title.focus({ preventScroll: true });
			if (title.getBoundingClientRect().height > 1) await showWhole(title);
		}
	}

	// A new rule opens its room in the list above the form (`grow`) and the page follows it, so the form holds its
	// place on the screen while the rule's card may stand cut at the screen's top: once it lands, the page settles
	// where nothing is cut (pass 9).
	/** The new rule's place in the list, once the list has it (RuleForm shows it above the form). */
	async function made(rule: Schemas['Rule']) {
		await load();
		await tick();
		return document.querySelector<HTMLElement>(`[data-rule="${rule.id}"]`);
	}

	// A link to the category form (from an uncategorized queue with no category to choose) lands in its name field:
	// the form only exists once the page has loaded, too late for the browser's own jump to the anchor.
	let fromQueue = $state(false);
	onMount(async () => {
		fromQueue = location.hash === '#new-category';
		await load();
		if (fromQueue) {
			await tick();
			// What stood in for the page leaves the flow as it fades, and the bar settles its height: measured before
			// that, the landing aimed 32 px off and left a label across the bar (pass 12).
			await Promise.all(
				document
					.getAnimations()
					.filter((a) => a.effect?.getTiming().iterations !== Infinity)
					.map((a) => a.finished.catch(() => undefined))
			);
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
	<p class="alert" role="alert" in:arrive out:placeholder>
		{loadError}
		<button class="secondary" onclick={(event) => retry(event.currentTarget, load)}
			>{t('app.try_again')}</button
		>
	</p>
{:else if loaded}
	<div class="stack rules-page" in:arrive>
		<div class="in-place">
			{#if rules.length}
				<ol class="rules" aria-label={t('rules.list')} in:swapIn out:vanish>
					{#each rules as rule, index (rule.id)}
						<li class="card rule" data-rule={rule.id} in:grow out:vanish animate:glide>
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
				<p in:swapIn out:vanish>{t('rules.none_yet')}</p>
			{/if}
		</div>

		<RuleForm {categories} {payees} {accounts} oncreated={made} />
		<CategoryForm oncreated={load} back={fromQueue} />
		<Changes {names} onapplied={load} />
	</div>
{:else}
	<p class="hint loading" out:placeholder>{t('app.loading')}</p>
{/if}

<style>
	/* While this page shows, the browser does not anchor the scroll: a rule moved up kept its place on screen by the
	   browser moving the page 218 px in one frame, its title left across the top bar; with only the list or the page's
	   content left out, what followed it became the anchor and the page jumped 32 px when the list got shorter
	   (pass 13). The rule glides and the page follows it. */
	:global(html:has(.rules-page):not(:has([data-growing]))) {
		overflow-anchor: none;
	}
	/* While a new rule grows, the browser anchors the scroll on what follows the list's cell (the form being used: with
	   only the list left out, the cell around it became the anchor; and not on the rail's column, as tall as the page
	   and never moving, which the browser took as the anchor at 1280), so it
	   stays still on screen as the rule opens its room above it: the focused button, pushed down, left the screen
	   (pass 17). Kept by hand, every frame read before the page moved showed the form 58 px off its place. */
	.rules-page > .in-place,
	:global(html:has(.rules-page [data-growing]) .wide-only) {
		overflow-anchor: none;
	}
	/* The cards' gap is the page's (.stack): a rule shown under the bar keeps the room the page's scroll padding
	   gives it, and the one above ends at the bar's edge. With 12 px, 4 px of it stood under the bar, and the landing
	   went up a whole card to show it (pass 14). */
	.rules {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: var(--space-lg);
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
	/* «Borrar» keeps one place on every card, at the row's end, with one move button or two: right after «Bajar» or
	   «Subir» it stood 23 px apart from card to card (pass 11). */
	.buttons > :global(.swap-cell) {
		margin-inline-start: auto;
	}
</style>
