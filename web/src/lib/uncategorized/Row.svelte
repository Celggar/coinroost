<script lang="ts">
	// One transaction waiting for a category, or the last one sorted. A sorted row stays where it was, tinted, until the
	// next choice (no timer: WCAG 2.2.1), says what it took and offers Deshacer.
	import type { Schemas } from '#lib/api/client.ts';
	import { day } from '#lib/format.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import { prefersReducedMotion } from 'svelte/motion';
	import { fade } from 'svelte/transition';
	import { DURATION, swapIn, vanish } from '#lib/motion.ts';
	import Arrow from '#lib/ui/Arrow.svelte';
	import { fadeAfter } from '#lib/ui/followers.ts';
	import { glideRoom } from '#lib/ui/hold-room.ts';
	import Money from '#lib/ui/Money.svelte';

	let {
		transaction,
		categories,
		took,
		busy,
		locked,
		onchoose,
		onundo
	}: {
		transaction: Schemas['Transaction'];
		categories: Schemas['Category'][];
		/** The name of the category it took, once sorted. */
		took: string | null;
		busy: boolean;
		/** Another row is being sent. */
		locked: boolean;
		onchoose: (id: string, select: HTMLSelectElement) => void;
		onundo: () => void;
	} = $props();

	const leg = $derived(transaction.legs[0]);

	// What it took may give its words a second line: where nothing may slide, what follows them fades in at its new place. It
	// stood 24 px lower from one frame to the next at 390 (pass 58).
	let words = $state<HTMLElement>();
	let lines = 0;
	$effect.pre(() => {
		void took;
		lines = words?.offsetHeight ?? 0;
	});
	$effect(() => {
		void took;
		if (!words || !lines) return;
		const moved = Math.abs(words.offsetHeight - lines) >= 4;
		if (!prefersReducedMotion.current) {
			if (moved) glideRoom(words, lines);
			return;
		}
		// Centred in the row, its first line went up 10 px for the second.
		words.animate({ opacity: [0, 1] }, { id: 'follow', duration: DURATION.medium });
		if (moved) fadeAfter(words, DURATION.medium, true);
	});

	/** Out, it gives back the line it held in a glide: gone in a frame at the end of the page, the whole screen moved 48 px (pass
	 * 61). Where nothing may slide it goes at once, behind the words' own fade. */
	function goes(node: HTMLElement) {
		if (prefersReducedMotion.current || !words) {
			node.style.display = 'none';
			return { duration: 1 };
		}
		const before = words.offsetHeight;
		node.style.display = 'none';
		const after = words.offsetHeight;
		node.style.display = '';
		glideRoom(words, before, after);
		return vanish(node);
	}

	// What moves the choice on a closed select: in Chromium each of these is a choice at once, and a choice moves on, so
	// «Su» for Supermercado saved Salario and the «u» fell on the next row (pass 40, WCAG 3.2.2). They open it instead.
	const MOVES = /^(.|Arrow(Up|Down|Left|Right)|Home|End|Page(Up|Down))$/u;

	function open(event: KeyboardEvent) {
		if (event.key === ' ' || event.ctrlKey || event.metaKey || event.altKey) return;
		if (!MOVES.test(event.key)) return;
		try {
			(event.currentTarget as HTMLSelectElement).showPicker();
			event.preventDefault();
		} catch {
			// Refused: the key does what the browser makes of it.
		}
	}
</script>

<div class="row" class:sorted={took}>
	<span class="date" data-whole>{day(transaction.date)}</span>
	<span class="description" data-whole bind:this={words}>
		<span class="name">{transaction.description}</span>
		{#if took}<span in:fade={{ duration: DURATION.medium }} out:goes
				><Arrow /><span class="took">{took}</span></span
			>{/if}
	</span>
	{#if leg}<span class="amount" data-whole
			><Money minor={leg.amount_minor} currency={leg.currency} /></span
		>{/if}
	{#if categories.length > 0}
		<div class="control in-place">
			{#if took}
				<!-- In, global: it comes with the row's change, not with a block of its own opening. Out, local: global, a row
				     leaving behind a filter's fade waited for this one and stood in the list as it came back. -->
				<div in:swapIn|global out:vanish>
					<button
						id="undo-{transaction.id}"
						class="secondary"
						aria-disabled={busy}
						onclick={() => busy || onundo()}>{t('uncategorized.undo')}</button
					>
				</div>
			{:else}
				<div in:swapIn|global out:vanish>
					<label>
						<span class="visually-hidden">{t('column.category')}</span>
						<select
							id="category-{transaction.id}"
							value=""
							disabled={locked}
							aria-disabled={busy}
							onkeydown={open}
							onchange={(event) => onchoose(event.currentTarget.value, event.currentTarget)}
						>
							<option value="" disabled>{t('uncategorized.choose')}</option>
							{#each categories as category (category.id)}
								<option value={category.id}>{category.name}</option>
							{/each}
						</select>
					</label>
				</div>
			{/if}
		</div>
	{/if}
</div>

<style>
	/* The list holds the columns and each row takes them, so every description starts and every amount ends at one
	   edge. On a phone: description and amount, the date, then the control. */
	.row {
		display: grid;
		grid-column: 1 / -1;
		grid-template-columns: subgrid;
		align-items: center;
		padding: var(--space-xs) var(--space-md);
		transition: background-color var(--duration-short) var(--ease-standard);
	}
	.date {
		font-size: var(--text-sm);
		color: var(--muted);
		white-space: nowrap;
	}
	.description {
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.sorted .name {
		font-weight: 600;
	}
	/* The space before it opens the gap on one line and goes at a wrap, so a line that starts with the arrow starts under the
	   text above; the drawing's own margin in its box comes back here: it stood 5 px to the right (pass 47). */
	.description :global(.arrow) {
		margin-inline-start: -0.16em;
	}
	.amount {
		justify-self: end;
		white-space: nowrap;
	}
	/* As tall as the select it takes the place of: on a phone 4 px less, and every row below stepped up as the row turned. */
	.control select,
	.control button {
		width: 100%;
		min-height: var(--field-height);
	}
	@media (max-width: 899px) {
		.row {
			grid-template-columns: minmax(0, 1fr) auto;
			gap: var(--space-2xs) var(--space-md);
			padding: var(--space-md);
		}
		.description {
			grid-column: 1;
			grid-row: 1;
		}
		.amount {
			grid-column: 2;
			grid-row: 1;
			align-self: start;
		}
		.date {
			grid-column: 1 / -1;
			grid-row: 2;
		}
		.control {
			grid-column: 1 / -1;
			grid-row: 3;
			margin-top: var(--space-xs);
		}
	}
</style>
