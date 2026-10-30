<script lang="ts">
	// A new set of recovery codes, shown once: two columns of codes, and buttons to copy them all or download them as
	// a text file (the way GitHub and Google hand them out). The file says what they are and when they were made.
	import { tag, t } from '#lib/i18n.svelte.ts';
	import { pop, vanish } from '#lib/motion.ts';

	let {
		codes,
		email = '',
		createdAt = ''
	}: { codes: string[]; email?: string; createdAt?: string } = $props();

	let copied = $state(false);
	let copiedTimer: ReturnType<typeof setTimeout> | undefined;

	const made = $derived(createdAt ? new Date(createdAt) : new Date());

	async function copy() {
		await navigator.clipboard.writeText(codes.join('\n'));
		copied = true;
		clearTimeout(copiedTimer);
		copiedTimer = setTimeout(() => (copied = false), 2000);
	}

	function download() {
		const date = made.toLocaleDateString(tag(), { dateStyle: 'long' });
		const lines = [
			t('recovery.file_title'),
			...(email ? [email] : []),
			t('recovery.file_made', { date }),
			'',
			...codes,
			'',
			t('recovery.file_note')
		];
		const file = new Blob([lines.join('\n') + '\n'], { type: 'text/plain;charset=utf-8' });
		const link = document.createElement('a');
		link.href = URL.createObjectURL(file);
		link.download = 'coinroost-recovery-codes.txt';
		link.click();
		setTimeout(() => URL.revokeObjectURL(link.href), 1000);
	}
</script>

<ol class="codes" aria-label={t('recovery.list')} in:pop|global={{ delay: 150, start: 0.95 }}>
	{#each codes as code (code)}
		<li><code>{code}</code></li>
	{/each}
</ol>
<div class="tools">
	<button type="button" class="secondary" onclick={copy} data-testid="recovery-copy">
		{#key copied}
			<span in:pop={{ start: 0.8 }} out:vanish>
				{copied ? t('recovery.copied_short') : t('recovery.copy')}
			</span>
		{/key}
	</button>
	<button type="button" class="secondary" onclick={download}>{t('recovery.download')}</button>
	<span class="visually-hidden" aria-live="polite">{copied ? t('recovery.copied') : ''}</span>
</div>

<style>
	.codes {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 8px 16px;
		margin: 0;
		padding: 14px 16px;
		list-style: none;
		border: 1px solid var(--field-border);
		border-radius: 12px;
		background: var(--surface);
	}
	code {
		font-family:
			ui-monospace, SFMono-Regular, Menlo, Consolas, 'Liberation Mono', 'DejaVu Sans Mono',
			monospace;
		font-size: 1rem;
		letter-spacing: 0.04em;
		white-space: nowrap;
	}
	.tools {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}
	.tools button {
		display: grid;
		place-items: center;
		min-height: 44px;
		flex: 1 1 0;
	}
	.tools button > :global(*) {
		grid-area: 1 / 1;
	}
</style>
