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
	// and would cut it (the chosen language had 5 px on each side at the rail's width). It is 320 px wide: the page's
	// column starts at 264 px up to a 1208 px window and then moves right, so some width always meets the panel's
	// edge; at 320 that is near 1344 px, while at 1280 the panel clearly overlaps the page (by 32 px) and at 1366 it
	// stays 11 px clear.
	let place = $state({ left: 0, bottom: 0 });
	// 13 px above the button: its focus ring reaches 5 px out (3 px wide, 2 px off) and keeps 8 px from the panel's
	// edge, as the log-out row's ring does inside it.
	const GAP = 13;

	const email = $derived(session.user?.email ?? '');
	const initial = $derived(email.slice(0, 1).toUpperCase());
	// The button shows the part before the @, which is what fits; the panel and the button's name say it whole.
	const shortName = $derived(email.split('@')[0]);
	// A long address breaks between the name and the domain before it breaks anywhere else.
	const domain = $derived(email.slice(shortName.length + 1));
	// The name's last two letters stay with the @: a name that filled the line exactly put the @ first on the next
	// one (P2 round 8).
	const nameHead = $derived(shortName.slice(0, -2));
	const nameTail = $derived(shortName.slice(-2));

	afterNavigate(() => (open = false));

	const outside = (target: EventTarget | null) => !root?.contains(target as Node);

	function toggle() {
		if (!open && button) {
			const box = button.getBoundingClientRect();
			place = { left: box.left, bottom: innerHeight - box.top + GAP };
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
			style:--gap="{GAP}px"
			in:pop={{ start: 0.92 }}
			out:vanish
		>
			<p class="who">
				<span>{t('nav.signed_in_as')}</span><span class="address"
					><span>{nameHead}<span class="glue">{nameTail}@</span></span><wbr /><span>{domain}</span
					></span
				>
			</p>
			<div class="language">
				<p class="label" aria-hidden="true">{t('more.language')}</p>
				<LanguageSwitch wide />
			</div>
			<div class="foot">
				<button class="leave" onclick={onleave}>
					<Icon name="logout" />
					<span>{t('nav.log_out')}</span>
				</button>
			</div>
			<span class="bridge" aria-hidden="true"></span>
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
		color: var(--muted);
		font-weight: 600;
	}
	/* At rest it reads like the rail's other rows, in grey; it takes the text colour under the pointer and while its
	   panel is open, so it never looks like a second current page. The initial keeps the text colour on its tint. */
	.trigger[aria-expanded='true'] {
		color: var(--text);
	}
	@media (hover: hover) {
		.trigger:hover {
			color: var(--text);
		}
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
		color: var(--text);
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
		width: 320px;
		display: grid;
		gap: var(--space-md);
		padding: var(--space-lg) var(--space-lg) var(--space-sm);
		/* Its own surface, which the tints inside it mix with: in dark the cards' fill made it read as a cut, not a layer. */
		--surface: var(--raised);
		border: 1px solid var(--raised-edge);
		border-radius: var(--radius-card);
		background: var(--surface);
		box-shadow: var(--raised-shadow);
		transform-origin: bottom left;
	}
	/* The gap down to the button belongs to the panel: through it showed the foot of «Seguridad», a 9 px strip that
	   took the click while the panel was open (P2 round 8). An element, not a pseudo-element, so checkers that read
	   what covers a target see it too. */
	.bridge {
		position: absolute;
		inset-inline: -1px;
		top: 100%;
		height: calc(var(--gap) + 1px);
	}
	/* The address on a line of its own, so it breaks only when it is longer than the panel. The panel's text starts
	   4 px in, where the log-out icon's stroke starts. */
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
	}
	/* The domain holds together while it fits the line, so a long address breaks after the @, not at a hyphen or a dot
	   in the domain; a name longer than the line breaks inside it and its end shares a line with the domain (as one
	   block it took the whole width and pushed the domain to a third line). */
	.address {
		overflow-wrap: anywhere;
	}
	.address > span:last-child {
		display: inline-block;
		max-width: 100%;
	}
	.glue {
		white-space: nowrap;
	}
	/* The dividers reach 8 px past the panel's text, as wide as the log-out row's hover tint below them. */
	.language {
		display: grid;
		gap: var(--space-xs);
		margin-inline: calc(-1 * var(--space-sm));
		padding: var(--space-md) var(--space-sm) 0;
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
	/* The same 12 px above both dividers. The log-out row sits in the middle between its divider and the panel's edge,
	   and its hover tint keeps 8 px from the divider, the edge and the sides, rounded to match the panel's corners. */
	.foot {
		margin-inline: calc(-1 * var(--space-sm));
		padding-top: var(--space-sm);
		border-top: 1px solid var(--border);
	}
	/* Its padding puts the icon's stroke (4 px into its box) on the panel's text line above. */
	.leave {
		--state-layer: color-mix(in oklab, var(--accent) 10%, transparent);
		justify-content: flex-start;
		gap: var(--space-md);
		width: 100%;
		min-height: 48px;
		padding: 0 var(--space-sm);
		border: 0;
		border-radius: calc(var(--radius-card) - var(--space-sm));
		background: none;
		color: var(--text);
	}
	/* Its focus ring is drawn inside it, clear of the divider above and the panel's edge below. */
	.leave:focus-visible {
		outline-offset: -3px;
	}
	/* Where more contrast is asked for, the panel and its dividers take the fields' stronger border, like the cards. */
	@media (prefers-contrast: more), (forced-colors: active) {
		.panel {
			box-shadow: none;
		}
		.panel,
		.language,
		.foot {
			border-color: var(--field-border);
		}
	}
</style>
