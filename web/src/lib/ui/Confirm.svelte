<script lang="ts">
	// A destructive action that asks first (FD6, FD9). The question takes the button's place: both sit in one cell,
	// so the one leaving fades over the one coming (Svelte makes it inert while it does, so it cannot be clicked), and
	// the focus moves to the button that took the pressed one's place: «keep» first, so a second Enter never confirms.
	import { tick } from 'svelte';
	import { pop, vanish } from '#lib/motion.ts';

	let {
		label,
		named,
		yes,
		no,
		busy = false,
		onconfirm
	}: {
		/** The button that asks, and its accessible name when the row needs one («Borrar Compras»). */
		label: string;
		named?: string;
		/** The danger button that does it, and the one that keeps things as they are. */
		yes: string;
		no: string;
		busy?: boolean;
		onconfirm: () => void;
	} = $props();

	let asking = $state(false);
	let ask = $state<HTMLButtonElement>();
	let keep = $state<HTMLButtonElement>();

	async function open(next: boolean) {
		asking = next;
		await tick();
		(next ? keep : ask)?.focus();
	}
</script>

<span class="swap-cell">
	{#if asking}
		<span class="choice" in:pop={{ start: 0.9 }} out:vanish>
			<button class="danger" onclick={onconfirm} disabled={busy}>{yes}</button>
			<button class="secondary" bind:this={keep} onclick={() => open(false)}>{no}</button>
		</span>
	{:else}
		<span class="choice" in:pop={{ start: 0.9 }} out:vanish>
			<button class="secondary" bind:this={ask} aria-label={named} onclick={() => open(true)}
				>{label}</button
			>
		</span>
	{/if}
</span>

<style>
	.swap-cell {
		display: inline-grid;
		align-items: center;
	}
	.choice {
		grid-area: 1 / 1;
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-sm);
	}
	/* Both answers as tall as the button that asked, and as round: side by side, their edges line up (his #158),
	   and the row keeps its height when the question opens. */
	.choice button {
		min-height: 44px;
		border-radius: var(--radius-button);
	}
</style>
