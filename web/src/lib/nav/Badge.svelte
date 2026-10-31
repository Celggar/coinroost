<script lang="ts">
	// A count on a destination (P1, N6): a number in a small pill that pops in, fades out at zero, and pops again when
	// the number changes. It is drawn only; the link it sits on is described by the sentence in `NavCounts`.
	import { pop, vanish } from '#lib/motion.ts';

	let { count, tone = 'bad' }: { count: number; tone?: 'bad' | 'warn' } = $props();
</script>

{#if count > 0}
	<span class="badge {tone}" aria-hidden="true" in:pop={{ start: 0.6 }} out:vanish>
		{#key count}<span in:pop={{ start: 0.8 }} out:vanish>{count > 99 ? '99+' : count}</span>{/key}
	</span>
{/if}

<style>
	.badge {
		display: inline-grid;
		place-items: center;
		min-width: 20px;
		height: 20px;
		padding: 0 var(--space-xs);
		border-radius: 999px;
		font-size: var(--text-xs);
		font-weight: 700;
		line-height: 1;
		font-variant-numeric: tabular-nums;
	}
	/* The old number fades out in the same cell the new one pops into. */
	.badge > span {
		display: inline-block;
		grid-area: 1 / 1;
	}
	.bad {
		background: var(--danger);
		color: var(--danger-text);
	}
	.warn {
		background: var(--warn-bg);
		color: var(--warn);
		box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--warn) 40%, transparent);
	}
</style>
