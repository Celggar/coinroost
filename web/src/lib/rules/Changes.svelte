<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { day, LIST, money } from '#lib/format.ts';
	import { plural, t } from '#lib/i18n.svelte.ts';
	import { followContent, glide, swapIn, swapOut } from '#lib/motion.ts';
	import { vanishRow } from '#lib/ui/hold-room.ts';
	import { focusOn } from '#lib/ui/focus.ts';
	import { showWhole } from '#lib/ui/landing.ts';
	import Arrow from '#lib/ui/Arrow.svelte';
	import Money from '#lib/ui/Money.svelte';
	import { notify } from '#lib/ui/notices.svelte.ts';
	import { refreshCounts } from '#lib/nav/counts.svelte.ts';
	import { categoryName, payeeName, type Names } from '#lib/rules/describe.ts';

	// `inline`: inside the offer that follows a choice, with no card of its own, the check already made.
	let {
		names,
		onapplied,
		inline = false
	}: { names: Names; onapplied: () => void; inline?: boolean } = $props();

	let preview = $state<Schemas['RulePreview'] | null>(null);
	let busy = $state(false);
	let card = $state<HTMLElement>();
	// Held while a new answer replaces the last one: the page moves once, to where it stands after (showWhole).
	let holding = $state(false);

	/** The rules changed: what the last check said no longer holds, so it leaves and «Ver cambios» asks again (pass 29). */
	export function forget() {
		preview = null;
	}

	/** Only what changes: the category, the payee, or both. */
	function what(change: Schemas['RuleChange']): string[] {
		const parts: string[] = [];
		const fromCategory = categoryName(change.before.category_id, names);
		const toCategory = categoryName(change.after.category_id, names);
		if (fromCategory !== toCategory)
			parts.push(t('changes.category', { from: fromCategory, to: toCategory }));
		const [fromPayee, toPayee] = [payeeName(change.before, names), payeeName(change.after, names)];
		if (fromPayee !== toPayee) parts.push(t('changes.payee', { from: fromPayee, to: toPayee }));
		if (parts.length === 0) parts.push(t('changes.only_rule'));
		return parts;
	}

	async function check() {
		if (busy) return;
		busy = true;
		const { data, error: failure, response } = await api.POST('/api/v1/rules/preview');
		busy = false;
		if (!data) {
			preview = null;
			notify(problem(failure, response?.status, t('changes.check_failed')), 'bad');
			return;
		}
		holding = true;
		preview = data;
		// The answer may land below the window or under the bottom bar: the focus goes to it and the page shows it.
		await tick();
		const answer = focusOn(data.changes.length ? '#changes-some' : '#changes-none');
		// The answer whole, down to the button that applies it: its first line alone left the changes below the screen.
		const send = data.changes.length
			? answer?.parentElement?.querySelector<HTMLElement>('.changes ~ .actions')
			: null;
		if (answer) await showWhole(answer, send ?? answer, card);
		holding = false;
	}

	async function apply() {
		if (!preview) return;
		busy = true;
		const {
			data,
			error: failure,
			response
		} = await api.POST('/api/v1/rules/apply', {
			params: { header: { 'If-Match': preview.tag } }
		});
		busy = false;
		if (!data) {
			notify(
				response?.status === 412
					? t('changes.stale')
					: problem(failure, response?.status, t('changes.apply_failed')),
				'bad'
			);
			return;
		}
		notify(plural(data.applied, 'changes.applied_one', 'changes.applied_other'));
		// Rules that set a category empty the uncategorized queue: Revisar's count follows at once (N6).
		refreshCounts();
		onapplied();
		// The apply button is gone with the changes: the check puts the focus on what the list says now.
		if (!inline) await check();
	}

	onMount(() => {
		if (inline) check();
	});
</script>

<!-- The card shrinks with what it holds instead of at once: at the page's end, the applied changes leaving pulled the
     page back 171 px in one frame (pass 14). -->
<section
	class="follows"
	class:card={!inline}
	aria-labelledby={inline ? undefined : 'changes-title'}
	aria-label={inline ? t('changes.title') : undefined}
	bind:this={card}
	use:followContent={holding}
>
	<div class="stack">
		{#if !inline}
			<h2 id="changes-title">{t('changes.title')}</h2>
			<p class="hint">{t('changes.hint')}</p>
			<div class="actions">
				<!-- aria-disabled, not disabled: a disabled button drops the focus to the page while it works. -->
				<button
					class="secondary"
					onclick={() => {
						check();
					}}
					aria-disabled={busy}>{t('changes.check')}</button
				>
			</div>
		{/if}
		{#if preview}
			<!-- `global`: these also enter when the first check opens the block around them, and leave when `forget` closes
			     it: a local transition does not run with its parent block, so the answer went in one frame (pass 30). In one
			     cell, the new answer waiting for the last: after «Aplicar» it stood under the list, then fell 355 px (pass 36). -->
			<div class="in-place">
				{#if preview.changes.length === 0}
					<p id="changes-none" tabindex="-1" in:swapIn|global out:swapOut|global>
						{t('changes.nothing')}
					</p>
				{:else}
					<div class="stack" in:swapIn|global out:swapOut|global>
						<p id="changes-some" tabindex="-1">
							{plural(preview.changes.length, 'changes.would_one', 'changes.would_other')}
						</p>
						<ul class="changes txn-rows">
							{#each preview.changes as change (change.transaction_id)}
								<li out:vanishRow animate:glide>
									<div class="txn-line">
										<span>{day(change.date)}</span>
										<span class="description">{change.description}</span>
										<Money minor={change.amount_minor} currency={change.currency} />
									</div>
									<!-- «De → a» with the drawn arrow: the font's «→» sat tiny and under the baseline (pass 23). -->
									<div class="hint">
										{#each what(change) as part, i (i)}{#if i}{LIST}{/if}{#each part.split(' → ') as side, j (j)}{#if j}<Arrow
													/>{/if}{side}{/each}{/each}
									</div>
								</li>
							{/each}
						</ul>
						<div class="actions">
							<button onclick={() => busy || apply()} aria-disabled={busy}>
								{plural(preview.changes.length, 'changes.apply_one', 'changes.apply_other')}
							</button>
						</div>
					</div>
				{/if}
			</div>
		{/if}
	</div>
</section>

<style>
	/* Clipped at the padding's edge, the buttons' focus rings stay whole; the content does not stretch to a height the
	   card holds. */
	.follows {
		position: relative;
		overflow: clip;
		align-content: start;
	}
	/* Inline it has no padding to keep them in, and an answer's ring stood cut to its right bar (pass 40): it lends itself
	   the room, up to the row above's padding so it never covers that row's button. */
	.follows:not(.card) {
		padding: var(--space-sm) var(--space-md);
		margin: calc(-1 * var(--space-sm)) calc(-1 * var(--space-md));
	}
	@media (prefers-reduced-motion: no-preference) {
		.follows {
			transition: height var(--duration-medium) var(--ease-standard);
		}
	}
	.changes {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: var(--space-sm);
	}
	.changes li {
		border-top: 1px solid var(--border);
		padding-top: var(--space-sm);
	}
</style>
