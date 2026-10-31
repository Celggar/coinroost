<script lang="ts">
	// The account's controls on a wide screen (P2): one button at the rail's foot, with the initial and the email,
	// opens a panel above it with the language and logging out. It is a disclosure, not an ARIA menu, because the
	// panel holds a radio group (WAI-ARIA APG disclosure pattern). The panel follows the button in the tab order;
	// Escape closes it and puts the focus back on the button, and a press or the focus elsewhere, or a page change,
	// closes it. It grows from the button; with reduced motion it only fades.
	import { afterNavigate } from '$app/navigation';
	import LanguageSwitch from '#lib/LanguageSwitch.svelte';
	import { t } from '#lib/i18n.svelte.ts';
	import { pop, vanish } from '#lib/motion.ts';
	import { session } from '#lib/session.svelte.ts';
	import Icon from './Icon.svelte';

	let { onleave }: { onleave: () => void } = $props();

	const id = $props.id();
	let open = $state(false);
	let root: HTMLElement | undefined = $state();
	let button: HTMLButtonElement | undefined = $state();

	const email = $derived(session.user?.email ?? '');
	const initial = $derived(email.slice(0, 1).toUpperCase());

	afterNavigate(() => (open = false));

	const outside = (target: EventTarget | null) => !root?.contains(target as Node);

	function key(event: KeyboardEvent) {
		if (!open || event.key !== 'Escape') return;
		event.preventDefault();
		open = false;
		button?.focus();
	}
</script>

<svelte:document
	onpointerdown={(event) => open && outside(event.target) && (open = false)}
	onfocusin={(event) => open && outside(event.target) && (open = false)}
	onkeydown={key}
/>

<div class="account" bind:this={root}>
	<button
		bind:this={button}
		class="trigger"
		aria-expanded={open}
		aria-controls={open ? `${id}-panel` : undefined}
		onclick={() => (open = !open)}
	>
		<span class="initial" aria-hidden="true">{initial}</span>
		<span class="email"><span class="visually-hidden">{t('nav.account')}: </span>{email}</span>
		<span class="chev" class:open aria-hidden="true"><Icon name="chev" size={20} /></span>
	</button>
	{#if open}
		<div class="panel" id="{id}-panel" in:pop={{ start: 0.92 }} out:vanish>
			<p class="who"><span>{t('nav.signed_in_as')}</span><span class="address">{email}</span></p>
			<div class="language">
				<p class="label" id="{id}-language" aria-hidden="true">{t('more.language')}</p>
				<LanguageSwitch wide />
			</div>
			<button class="leave" onclick={onleave}>
				<Icon name="logout" />
				<span>{t('nav.log_out')}</span>
			</button>
		</div>
	{/if}
</div>

<style>
	/* A grid item sizes to its content's widest line, and the email never wraps on the button: without this the
	   button and its panel grew past the rail's edge. */
	.account {
		position: relative;
		min-width: 0;
	}
	.trigger {
		--state-layer: color-mix(in oklab, var(--accent) 10%, transparent);
		width: 100%;
		min-height: 52px;
		justify-content: flex-start;
		gap: var(--space-md);
		padding: 0 var(--space-sm);
		border: 0;
		border-radius: 999px;
		background: none;
		color: var(--text);
		font-weight: 600;
	}
	.initial {
		flex: none;
		display: grid;
		place-items: center;
		width: 36px;
		height: 36px;
		border-radius: 50%;
		background: color-mix(in oklab, var(--accent) 18%, var(--surface));
		font-weight: 700;
	}
	/* A long address ends in an ellipsis on the button; the panel shows it whole. */
	.email {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		text-align: start;
		font-size: var(--text-sm);
	}
	/* The chevron points up to where the panel opens, and down while it is open. */
	.chev {
		display: grid;
		color: var(--muted);
		transform: rotate(-90deg);
	}
	.chev.open {
		transform: rotate(90deg);
	}
	/* It turns only where motion is welcome; otherwise it flips at once. */
	@media (prefers-reduced-motion: no-preference) {
		.chev {
			transition: transform var(--duration-short) var(--ease-standard);
		}
	}
	/* Above the button, as wide as the rail's foot: the rail scrolls, so the panel stays inside it. */
	.panel {
		position: absolute;
		left: 0;
		right: 0;
		bottom: calc(100% + var(--space-sm));
		z-index: 20;
		display: grid;
		gap: var(--space-md);
		padding: var(--space-lg) var(--space-md) var(--space-sm);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		background: var(--surface);
		box-shadow: var(--card-shadow);
		transform-origin: bottom left;
	}
	/* The address on a line of its own, so it breaks only when it is longer than the panel. */
	.who {
		display: grid;
		gap: var(--space-2xs);
		padding: 0 var(--space-xs);
		font-size: var(--text-sm);
		color: var(--muted);
	}
	.address {
		color: var(--text);
		font-weight: 600;
		overflow-wrap: anywhere;
	}
	.language {
		display: grid;
		gap: var(--space-xs);
		padding-top: var(--space-md);
		border-top: 1px solid var(--border);
	}
	.label {
		padding: 0 var(--space-xs);
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--muted);
	}
	/* The chosen language takes the accent tint of the rail's current page. */
	.language :global(.language) {
		--language-chosen: color-mix(in oklab, var(--accent) 14%, var(--surface));
	}
	.leave {
		--state-layer: color-mix(in oklab, var(--accent) 10%, transparent);
		justify-content: flex-start;
		gap: var(--space-md);
		min-height: 48px;
		padding: 0 var(--space-md);
		border: 0;
		border-top: 1px solid var(--border);
		border-radius: 0 0 var(--radius-button) var(--radius-button);
		background: none;
		color: var(--text);
	}
</style>
