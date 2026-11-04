<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { prefersReducedMotion } from 'svelte/motion';
	import { fade, slide } from 'svelte/transition';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import Changes from '#lib/rules/Changes.svelte';
	import Tick from '#lib/ui/Tick.svelte';
	import RuleForm from '#lib/rules/RuleForm.svelte';
	import { actions, conditions, names as nameMaps } from '#lib/rules/describe.ts';
	import { LIST } from '#lib/format.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import Confirm from '#lib/ui/Confirm.svelte';
	import { focusMade, focusOn, holdFocus, retry } from '#lib/ui/focus.ts';
	import { holdRoom } from '#lib/ui/hold-room.ts';
	import { showWhole } from '#lib/ui/landing.ts';
	import { notify } from '#lib/ui/notices.svelte.ts';
	import Sheet from '#lib/ui/Sheet.svelte';
	import {
		arrive,
		DURATION,
		EASE,
		glide,
		grow,
		placeholder,
		pop,
		swapIn,
		vanish
	} from '#lib/motion.ts';

	let rules = $state<Schemas['Rule'][]>([]);
	let categories = $state<Schemas['Category'][]>([]);
	let payees = $state<Schemas['Payee'][]>([]);
	let accounts = $state<Schemas['Account'][]>([]);
	let loadError = $state('');
	let loaded = $state(false);
	let busy = $state(false);
	let title = $state<HTMLHeadingElement>();
	let adding = $state(false);
	let phone = $state(false);

	const open = (node: Element) =>
		prefersReducedMotion.current
			? fade(node, { duration: DURATION.short })
			: slide(node, { duration: DURATION.medium, easing: EASE.enter });

	const names = $derived(nameMaps(categories, payees, accounts));

	// What the next load runs once the new lists are in hand, just before they replace the old ones; once.
	let arriving: (() => void) | undefined;

	let changes = $state<Changes>();

	async function load() {
		const before = arriving;
		arriving = undefined;
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
		before?.();
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
		} else changes?.forget();
		// The list holds the focus while its rows change places, so the row that moves takes none out with it.
		arriving = () => holdFocus(document.querySelector('ol.rules'));
		await load();
		// The focus stays on the pressed button while it moves (aria-disabled, pass 18) and follows the rule, to the same
		// button, or to its other one when the rule reached the top or the bottom (the one leaving is inert, so skipped).
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
		} else changes?.forget();
		await holdRoom(document.querySelector('.rules-page > .in-place'), load);
		// The rule and its buttons are gone: the focus goes to the page's title. Where the title shows (from 900 px) the
		// page glides to it; on a phone the top bar holds the title and the page stays. Focused bare, the page jumped to
		// its start in one frame (pass 15).
		if (response.ok && title) {
			title.focus({ preventScroll: true });
			if (title.getBoundingClientRect().height > 1) await showWhole(title);
		}
	}

	// A new rule opens its room in the list (`grow`) as the form closes: once it lands, the page settles where nothing
	// is cut (pass 9).
	/** The form closes and the new rule, in its place in the list, takes the focus and shows whole. */
	async function made(rule: Schemas['Rule']) {
		changes?.forget();
		await load();
		adding = false;
		await tick();
		const row = focusMade(`[data-rule="${rule.id}"]`);
		if (row) await showWhole(row);
	}

	/** Opens the form: in place on a wide screen, as a full-screen dialog on a phone, on its first field. */
	async function add() {
		phone = matchMedia('(max-width: 899px)').matches;
		adding = true;
		await tick();
		focusOn('#rule-form input');
	}

	async function close() {
		adding = false;
		await tick();
		focusOn('#new-rule');
	}

	onMount(load);
</script>

<svelte:head>
	<title>{t('app.title', { page: t('rules.title') })}</title>
</svelte:head>

<div class="title-bar">
	<h1 tabindex="-1" bind:this={title}>{t('rules.title')}</h1>
	{#if loaded && rules.length && !adding}
		<button id="new-rule" onclick={add} in:vanish out:vanish>+ {t('rules.add')}</button>
	{/if}
</div>
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
										onclick={() => busy || move(rule, rule.position - 1)}
										aria-disabled={busy}
										aria-label={t('rules.up_named', { name: rule.name })}
										in:pop={{ start: 0.9 }}
										out:vanish>{t('rules.up')}</button
									>
								{/if}
								{#if index < rules.length - 1}
									<button
										class="secondary"
										data-move="{rule.id}-down"
										onclick={() => busy || move(rule, rule.position + 1)}
										aria-disabled={busy}
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

		{#if adding && phone}
			<Sheet
				title={t('ruleform.title')}
				form="rule-form"
				action={t('ruleform.save')}
				onclose={close}
			>
				<RuleForm {categories} {payees} {accounts} sheet oncreated={made} />
			</Sheet>
		{:else if adding || rules.length === 0}
			<div transition:open>
				<RuleForm
					{categories}
					{payees}
					{accounts}
					oncreated={made}
					oncancel={rules.length ? close : undefined}
				/>
			</div>
		{/if}
		<Changes bind:this={changes} {names} onapplied={load} />
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
