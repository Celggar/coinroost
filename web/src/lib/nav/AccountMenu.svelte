<script lang="ts">
	// The account's controls on a wide screen (P2): one button at the rail's foot, with the initial and the name the
	// email starts with, opens a panel above it with the whole email, the language and logging out. It is a
	// disclosure, not an ARIA menu, because the panel holds a radio group (WAI-ARIA APG disclosure pattern). The panel
	// follows the button in the tab order; Escape closes it and puts the focus back on the button, and a press or the
	// focus elsewhere, a page change or a resize closes it. It grows from the button; with reduced motion it only fades.
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
	// Where the panel stands: fixed to the window above the button, so it can be wider than the rail, which scrolls
	// and would cut it (the chosen language had 5 px on each side at the rail's width).
	let place = $state({ left: 0, bottom: 0 });

	const email = $derived(session.user?.email ?? '');
	const initial = $derived(email.slice(0, 1).toUpperCase());
	// The button shows the part before the @, which is what fits; the panel and the button's name say it whole.
	const shortName = $derived(email.split('@')[0]);

	afterNavigate(() => (open = false));

	const outside = (target: EventTarget | null) => !root?.contains(target as Node);

	function toggle() {
		if (!open && button) {
			const box = button.getBoundingClientRect();
			place = { left: box.left, bottom: innerHeight - box.top + 8 };
		}
		open = !open;
	}

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
<svelte:window onresize={() => (open = false)} />

<div class="account" bind:this={root}>
	<button
		bind:this={button}
		class="trigger"
		aria-expanded={open}
		aria-controls={open ? `${id}-panel` : undefined}
		onclick={toggle}
	>
		<span class="initial" aria-hidden="true">{initial}</span>
		<span class="name"
			><span class="visually-hidden">{t('nav.account')}: {email}</span><span aria-hidden="true"
				>{shortName}</span
			></span
		>
		<span class="chev" class:open aria-hidden="true"><Icon name="chev" size={20} /></span>
	</button>
	{#if open}
		<div
			class="panel"
			id="{id}-panel"
			style:left="{place.left}px"
			style:bottom="{place.bottom}px"
			in:pop={{ start: 0.92 }}
			out:vanish
		>
			<p class="who"><span>{t('nav.signed_in_as')}</span><span class="address">{email}</span></p>
			<div class="language">
				<p class="label" aria-hidden="true">{t('more.language')}</p>
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
	/* A grid item sizes to its content's widest line, and the name never wraps on the button: without this the
	   button grew past the rail's edge. */
	.account {
		min-width: 0;
	}
	/* Lined up with the rail's rows above it: the initial centred on their icons, the name on their labels, the
	   chevron at their badges' edge. */
	.trigger {
		--state-layer: color-mix(in oklab, var(--accent) 10%, transparent);
		width: 100%;
		min-height: 48px;
		justify-content: flex-start;
		gap: var(--space-sm);
		padding: 0 var(--space-lg) 0 var(--space-md);
		border: 0;
		border-radius: 999px;
		background: none;
		color: var(--text);
		font-weight: 600;
	}
	/* Stronger than the hover's tint, so the circle stays apart from it in both themes. */
	.initial {
		flex: none;
		display: grid;
		place-items: center;
		width: 32px;
		height: 32px;
		border-radius: 50%;
		background: color-mix(in oklab, var(--accent) 30%, var(--surface));
		font-size: var(--text-sm);
		font-weight: 700;
	}
	/* Forced colors drop the tint: a border keeps the initial in its circle. */
	@media (forced-colors: active) {
		.initial {
			border: 1px solid CanvasText;
		}
	}
	.name {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		text-align: start;
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
	.panel {
		position: fixed;
		z-index: 30;
		width: 288px;
		display: grid;
		gap: var(--space-md);
		padding: var(--space-lg) var(--space-lg) var(--space-md);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		background: var(--surface);
		box-shadow: var(--card-shadow);
		transform-origin: bottom left;
	}
	/* The address on a line of its own, so it breaks only when it is longer than the panel. */
	/* The panel's text and the log-out icon start 4 px in, on one line. */
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
		margin-top: var(--space-xs);
		padding: 0 var(--space-xs);
		border: 0;
		border-top: 1px solid var(--border);
		border-radius: 0;
		background: none;
		color: var(--text);
	}
</style>
