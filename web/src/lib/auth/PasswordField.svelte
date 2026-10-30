<script lang="ts">
	// A password field with a Show/Hide button (GOV.UK's password input): the button sits outside the label so the
	// field keeps a single name, the field keeps its autocomplete, and the page hides the password again before it
	// submits, so the browser never keeps it as plain text.
	import { tick } from 'svelte';
	import { t } from '#lib/i18n.svelte.ts';
	import { pop, vanish } from '#lib/motion.ts';

	let {
		label,
		hint,
		autocomplete,
		minlength,
		value = $bindable('')
	}: {
		label: string;
		hint?: string;
		autocomplete: 'current-password' | 'new-password';
		minlength?: number;
		value?: string;
	} = $props();

	const id = $props.id();
	let shown = $state(false);
	let input: HTMLInputElement | undefined = $state();

	/** Show or hide. Safari keeps a field it filled with a strong password in dots whatever its type (WebKit's
	 * autofill-and-obscured state, `-webkit-text-security: disc !important`); a value written by the page ends that
	 * state (WebKit's HTMLInputElement::setValue), so showing writes the same value back. */
	async function toggle() {
		shown = !shown;
		if (!shown || !input) return;
		await tick();
		const typed = input.value;
		input.value = '';
		input.value = typed;
	}

	/** Back to dots: the page calls it before it submits. */
	export function hide() {
		shown = false;
	}
</script>

<div class="password">
	<label for={id}>{label}</label>
	{#if hint}<span class="hint" id="{id}-hint">{hint}</span>{/if}
	<div class="field">
		<input
			bind:this={input}
			{id}
			type={shown ? 'text' : 'password'}
			{autocomplete}
			{minlength}
			required
			autocapitalize="none"
			spellcheck="false"
			aria-describedby={hint ? `${id}-hint` : undefined}
			{value}
			oninput={(event) => (value = event.currentTarget.value)}
		/>
		<button type="button" class="reveal" aria-controls={id} onclick={toggle}>
			{#key shown}
				<span in:pop={{ start: 0.85 }} out:vanish
					>{shown ? t('password.hide') : t('password.show')}</span
				>
			{/key}
		</button>
	</div>
	<span class="visually-hidden" aria-live="polite">{shown ? t('password.shown') : ''}</span>
</div>

<style>
	.password {
		display: grid;
		gap: 6px;
	}
	.field {
		position: relative;
	}
	input {
		padding-right: 96px;
	}
	.reveal {
		position: absolute;
		top: 4px;
		right: 4px;
		bottom: 4px;
		min-height: 0;
		padding: 0 12px;
		display: grid;
		place-items: center;
		border: 0;
		border-radius: 8px;
		background: transparent;
		color: var(--link);
		font-weight: 600;
		font-size: 0.9rem;
		--state-layer: color-mix(in oklab, var(--accent) 12%, transparent);
	}
	.reveal > :global(*) {
		grid-area: 1 / 1;
	}
</style>
