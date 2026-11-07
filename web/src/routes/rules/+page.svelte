<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import Changes from '#lib/rules/Changes.svelte';
	import Tick from '#lib/ui/Tick.svelte';
	import RuleForm from '#lib/rules/RuleForm.svelte';
	import { actions, conditions, names as nameMaps } from '#lib/rules/describe.ts';
	import { LIST } from '#lib/format.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import Confirm from '#lib/ui/Confirm.svelte';
	import { focusMade, focusOn, holdFocus, retry } from '#lib/ui/focus.ts';
	import { firstRoom, follow, holdRoom, openRoom, reshape } from '#lib/ui/hold-room.ts';
	import { showWhole } from '#lib/ui/landing.ts';
	import { notify } from '#lib/ui/notices.svelte.ts';
	import Sheet from '#lib/ui/Sheet.svelte';
	import { arrive, FADED, glide, grow, placeholder, pop, swapIn, vanish } from '#lib/motion.ts';

	let rules = $state<Schemas['Rule'][]>([]);
	let categories = $state<Schemas['Category'][]>([]);
	let payees = $state<Schemas['Payee'][]>([]);
	let accounts = $state<Schemas['Account'][]>([]);
	let loadError = $state('');
	let loaded = $state(false);
	let busy = $state(false);
	let title = $state<HTMLHeadingElement>();
	let adding = $state(false);
	let sheet = $state<Sheet>();
	let phone = $state(false);

	const names = $derived(nameMaps(categories, payees, accounts));

	const MOVES = {
		up: { by: -1, word: 'rules.up', named: 'rules.up_named' },
		down: { by: 1, word: 'rules.down', named: 'rules.down_named' }
	} as const;
	const ways = (index: number) =>
		(['up', 'down'] as const).filter((way) =>
			way === 'up' ? index > 0 : index < rules.length - 1
		);

	/** A move button sideways, when the other one comes or goes beside it: 83 px in a frame as a rule left the top (pass 65
	 * sweep). Only sideways, as its row already takes it up or down. */
	function aside(node: Element, { from, to }: { from: DOMRect; to: DOMRect }) {
		return glide(node, { from: new DOMRect(from.left, to.top), to });
	}

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
		const list = document.querySelector<HTMLElement>('.rules-page > .in-place');
		const first = !rules.length;
		const before = list?.offsetHeight ?? 0;
		// The page takes its new height behind the same dim as its landing: the rows went out and back, then out again (pass 57).
		await reshape(load);
		sheet?.letGo();
		adding = false;
		await tick();
		if (first) firstRoom(list, before);
		const row = focusMade(`[data-rule="${rule.id}"]`);
		if (row) await showWhole(row);
	}

	/** Opens the form: in place on a wide screen, as a full-screen dialog on a phone, on its first field. */
	async function add() {
		phone = matchMedia('(max-width: 899px)').matches;
		adding = true;
		await tick();
		const field = focusOn('#rule-form input');
		// Under a long list it opens below the window, the focus out of sight (pass 48).
		if (field && !phone) await showWhole(field.closest('form') ?? field);
	}

	async function close() {
		sheet?.letGo();
		adding = false;
		await tick();
		const opener = focusOn('#new-rule');
		if (opener) await showWhole(opener);
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
						<li
							class="card rule"
							data-rule={rule.id}
							in:grow
							out:vanish
							animate:glide
							onintrostart={follow}
						>
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
								{#each ways(index) as way (way)}
									<button
										class="secondary"
										data-move="{rule.id}-{way}"
										onclick={() => busy || move(rule, rule.position + MOVES[way].by)}
										aria-disabled={busy}
										aria-label={t(MOVES[way].named, { name: rule.name })}
										in:pop={{ start: 0.9 }}
										out:vanish
										animate:aside>{t(MOVES[way].word)}</button
									>
								{/each}
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
				bind:this={sheet}
				title={t('ruleform.title')}
				form="rule-form"
				action={t('ruleform.save')}
				onclose={close}
			>
				<RuleForm {categories} {payees} {accounts} sheet oncreated={made} />
			</Sheet>
		{:else if adding || rules.length === 0}
			<!-- With the last rule gone it opens by itself: it waits for that rule to fade, or it pushed what follows the list
			     down under it. -->
			<div
				transition:openRoom={{ delay: rules.length ? 0 : FADED }}
				onintrostart={rules.length ? follow : undefined}
				onoutroend={follow}
			>
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
	   (pass 13). The rule glides and the page follows it. A new rule too: anchored on the field in use, the rows above took a
	   row's height up as the list measured them, and came down 222 px in a frame (pass 62's sweep). */
	/* A column, not the stack's grid: a form that opens with its gap starts at a height of 0 and a margin of −16 px, and a grid
	   row cannot be less than 0, so what follows stood 16 px lower before anything opened (pass 46's brief, P5). */
	.rules-page {
		display: flex;
		flex-direction: column;
	}
	:global(html:has(.rules-page)) {
		overflow-anchor: none;
	}
	/* The cards' gap is the page's (.stack): a rule shown under the bar keeps the room the page's scroll padding
	   gives it, and the one above ends at the bar's edge. With 12 px, 4 px of it stood under the bar, and the landing
	   went up a whole card to show it (pass 14). A column: a grid row cannot be less than 0, and a new rule's gap stood whole
	   from its first frame, the form under it 16 px down at once (pass 62's sweep). */
	.rules {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
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
	/* A move button that goes is taken out of the flow and held in its place by a transform, which the buttons' transition
	   eased: 83 px to the row's start and back, under reduced motion too (pass 65 sweep). */
	.buttons > :global(button[style*='position: absolute']) {
		transition: none;
	}
	/* «Borrar» keeps one place on every card, at the row's end, with one move button or two: right after «Bajar» or
	   «Subir» it stood 23 px apart from card to card (pass 11). `end`: its cell opens to the left for the question, and at
	   the cell's start it went 131 px with it in a frame, under the question's words (pass 65). */
	.buttons > :global(.swap-cell) {
		margin-inline-start: auto;
		justify-items: end;
	}
	/* Where its question does not fit beside two moves, «Borrar» keeps a line of its own: asking, the row took one more at
	   once, 52 px in a frame for every rule under it (pass 51). */
	@media (max-width: 479px) {
		.buttons > :global(.swap-cell) {
			flex-basis: 100%;
		}
	}
</style>
