<script lang="ts">
	// What opens the feedback panel, on every screen of the app: an icon button at the end of the phone's top bar, a row
	// in the rail's foot on a wide screen. Its badge counts the screenshots waiting in the draft. The panel is the
	// layout's, one for the app; closed, it gives the focus back to the button that opened it.
	import { plural, t } from '#lib/i18n.svelte.ts';
	import Badge from '#lib/nav/Badge.svelte';
	import Icon from '#lib/nav/Icon.svelte';
	import { draft, openPanel } from './draft.svelte.ts';

	let { place }: { place: 'bar' | 'rail' } = $props();

	const waiting = $derived(draft.shots.length);
	const id = $props.id();
</script>

<button
	type="button"
	class={place}
	aria-label={place === 'bar' ? t('feedback.button_label') : undefined}
	aria-describedby={waiting ? `${id}-waiting` : undefined}
	aria-haspopup="dialog"
	onclick={(event) => openPanel(event.currentTarget)}
>
	<span class="mark">
		<Icon name="feedback" />
		{#if place === 'bar'}<span class="corner"><Badge count={waiting} tone="accent" /></span>{/if}
	</span>
	{#if place === 'rail'}
		<span class="label">{t('feedback.button')}</span>
		<Badge count={waiting} tone="accent" />
	{/if}
</button>
<span id="{id}-waiting" hidden
	>{plural(waiting, 'feedback.waiting_one', 'feedback.waiting_other')}</span
>

<style>
	button {
		--state-layer: color-mix(in oklab, var(--accent) 10%, transparent);
		position: relative;
		isolation: isolate;
		display: flex;
		align-items: center;
		border: 0;
		background: transparent;
		color: var(--muted);
		font: inherit;
		font-weight: 400;
	}
	@media (hover: hover) {
		button:hover {
			color: var(--text);
		}
	}
	.mark {
		position: relative;
		display: inline-grid;
	}
	/* The phone's top bar: a 48 px icon button at its end, the badge on the icon's corner as on the bottom bar. */
	.bar {
		justify-content: center;
		min-width: 48px;
		min-height: 48px;
		margin-left: auto;
		padding: 0;
		border-radius: 999px;
		color: var(--text);
		/* The bar is always on screen, but it stands inside the page's scroll padding, so a browser that focuses the button
		   takes it for hidden and moves the page: 386 px up at each Tab, and on every close of the panel (pass 77), as
		   the bottom bar's links did (pass 35). */
		scroll-margin-top: -100vh;
	}
	/* The bar's 56 px leave 4 px over the 48 px button: the ring goes inside its edge, or half its top stroke stood off
	   the screen (pass 77). */
	.bar:focus-visible {
		outline-offset: -3px;
	}
	.corner {
		position: absolute;
		top: -6px;
		left: 14px;
	}
	/* The rail's foot: a row like «Seguridad» under it. */
	.rail {
		gap: var(--space-md);
		width: 100%;
		min-height: 48px;
		padding: 0 var(--space-lg);
		border-radius: 999px;
	}
	.label {
		flex: 1;
		text-align: start;
	}
</style>
