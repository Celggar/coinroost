<script lang="ts">
	// The feedback panel: what kind of report, a message, and screenshots, each with its own note. A full-screen dialog on
	// a phone, a side sheet on a wide screen. «Capturar esta pantalla» draws the screen under the panel; the draft
	// outlives the panel, so a person can close it, go to another screen and capture that one into the same report.
	import { tick } from 'svelte';
	import { api, problem } from '#lib/api/client.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import Icon from '#lib/nav/Icon.svelte';
	import { glide, grow, vanish } from '#lib/motion.ts';
	import { notify } from '#lib/ui/notices.svelte.ts';
	import Field from '#lib/ui/Field.svelte';
	import FormError from '#lib/ui/FormError.svelte';
	import Sheet from '#lib/ui/Sheet.svelte';
	import { base64, captureScreen, fromFile } from './capture.ts';
	import {
		MAX_MESSAGE,
		MAX_NOTE,
		MAX_SHOTS,
		addShot,
		closePanel,
		draft,
		removeShot,
		startOver
	} from './draft.svelte.ts';

	let sheet = $state<Sheet>();
	let picker = $state<HTMLInputElement>();
	let error = $state('');
	let busy = $state(false);
	let capturing = $state(false);
	const full = $derived(draft.shots.length >= MAX_SHOTS);

	/** Before the panel goes: it lets the page have the focus back as it starts to leave. */
	function close() {
		sheet?.letGo();
		closePanel();
	}

	async function focusNote(key: number) {
		await tick();
		document.querySelector<HTMLTextAreaElement>(`[data-note="${key}"]`)?.focus();
	}

	async function capture() {
		error = '';
		capturing = true;
		try {
			const shot = await addShot(await captureScreen('dialog'));
			await focusNote(shot.key);
		} catch {
			error = t('feedback.capture_failed');
		} finally {
			capturing = false;
		}
	}

	async function attach(files: FileList | null) {
		error = '';
		let last = 0;
		for (const file of [...(files ?? [])].slice(0, MAX_SHOTS - draft.shots.length)) {
			try {
				last = (await addShot(await fromFile(file))).key;
			} catch {
				error = t('feedback.not_image', { name: file.name });
			}
		}
		if (picker) picker.value = '';
		if (last) await focusNote(last);
	}

	function remove(key: number, index: number) {
		removeShot(key);
		// The focus goes to what took the row's place: the next row's note, the one before, or the add button.
		const next = draft.shots[index] ?? draft.shots[index - 1];
		if (next) focusNote(next.key);
		else document.querySelector<HTMLButtonElement>('[data-capture]')?.focus();
	}

	async function send(event: SubmitEvent) {
		event.preventDefault();
		if (busy) return;
		error = '';
		if (!draft.message.trim() && draft.shots.length === 0) {
			error = t('feedback.empty');
			return;
		}
		busy = true;
		try {
			const shots = await Promise.all(
				draft.shots.map(async (s) => ({ image: await base64(s.blob), note: s.note }))
			);
			const { error: failed, response } = await api.POST('/api/v1/feedback', {
				body: {
					report_id: draft.reportId,
					kind: draft.kind,
					message: draft.message,
					shots,
					context: {
						path: location.pathname + location.search,
						viewport_width: innerWidth,
						viewport_height: innerHeight,
						color_scheme: matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light',
						language: document.documentElement.lang || 'es',
						user_agent: navigator.userAgent.slice(0, 512)
					}
				}
			});
			if (failed) {
				error = problem(failed, response?.status, t('feedback.what'));
				return;
			}
			startOver();
			notify(t('feedback.sent'));
			close();
		} catch {
			error = problem(undefined, undefined, t('feedback.what'));
		} finally {
			busy = false;
		}
	}
</script>

<Sheet
	bind:this={sheet}
	side
	title={t('feedback.title')}
	form="feedback-form"
	action={busy ? t('feedback.sending') : t('feedback.send')}
	onclose={close}
>
	<form id="feedback-form" class="panel" novalidate onsubmit={send}>
		<Field label={t('feedback.kind')} name="kind">
			{#snippet children(wiring)}
				<select {...wiring} bind:value={draft.kind}>
					<option value="problem">{t('feedback.kind_problem')}</option>
					<option value="idea">{t('feedback.kind_idea')}</option>
				</select>
			{/snippet}
		</Field>
		<Field label={t('feedback.message')} name="message" hint={t('feedback.message_hint')}>
			{#snippet children(wiring)}
				<textarea {...wiring} rows="4" maxlength={MAX_MESSAGE} bind:value={draft.message}
				></textarea>
			{/snippet}
		</Field>

		<section class="shots" aria-labelledby="feedback-shots">
			<h3 id="feedback-shots">{t('feedback.shots')}</h3>
			<p class="hint">{full ? t('feedback.full') : t('feedback.shots_hint')}</p>
			<!-- Always here, empty or not, so the first row comes into a list that is already there and plays its own transition. -->
			<ol>
				{#each draft.shots as shot, i (shot.key)}
					<li in:grow out:vanish animate:glide>
						<img src={shot.url} alt={t('feedback.shot_alt', { n: i + 1 })} />
						<div class="note">
							<Field label={t('feedback.note', { n: i + 1 })} name="note-{shot.key}">
								{#snippet children(wiring)}
									<textarea
										{...wiring}
										data-note={shot.key}
										rows="3"
										maxlength={MAX_NOTE}
										bind:value={shot.note}></textarea>
								{/snippet}
							</Field>
							<button
								type="button"
								class="link"
								aria-label={t('feedback.remove_label', { n: i + 1 })}
								onclick={() => remove(shot.key, i)}>{t('feedback.remove')}</button
							>
						</div>
					</li>
				{/each}
			</ol>
			<div class="add">
				<button
					type="button"
					class="secondary"
					data-capture
					disabled={full || capturing}
					onclick={capture}
					><Icon name="camera" size={20} />{capturing
						? t('feedback.capturing')
						: t('feedback.capture')}</button
				>
				<button type="button" class="secondary" disabled={full} onclick={() => picker?.click()}
					><Icon name="image" size={20} />{t('feedback.attach')}</button
				>
				<input
					bind:this={picker}
					type="file"
					accept="image/*"
					multiple
					hidden
					onchange={(event) => attach(event.currentTarget.files)}
				/>
			</div>
		</section>

		<FormError text={error} />
		<p class="hint">{t('feedback.private')}</p>
		<a href="/feedback" onclick={close}>{t('feedback.see_sent')}</a>
	</form>
</Sheet>

<style>
	.panel {
		display: grid;
		gap: var(--space-lg);
	}
	textarea {
		padding-block: var(--space-sm);
		line-height: 1.4;
		resize: vertical;
	}
	h3 {
		margin: 0;
		font-size: var(--text-md);
	}
	.shots {
		display: grid;
		gap: var(--space-sm);
	}
	.shots > .hint {
		margin: 0;
	}
	ol:empty {
		display: none;
	}
	ol {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: var(--space-md);
	}
	li {
		display: grid;
		grid-template-columns: 96px minmax(0, 1fr);
		gap: var(--space-md);
		align-items: start;
	}
	img {
		width: 96px;
		max-height: 192px;
		object-fit: cover;
		object-position: top;
		border: 1px solid var(--border);
		border-radius: var(--radius-field);
		background: var(--bg);
	}
	/* The note takes the row's width; «Quitar» stays its own size under it. */
	.note {
		display: grid;
		gap: var(--space-xs);
	}
	.note > button {
		justify-self: start;
	}
	/* The two ways to add share the width: one column in a phone's panel, side by side where both fit. */
	.add {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
		gap: var(--space-sm);
	}
	.add button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-sm);
	}
</style>
