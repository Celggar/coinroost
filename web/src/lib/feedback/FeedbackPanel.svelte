<script lang="ts">
	// The feedback panel: what kind of report, a message, and screenshots, each with its own note. A full-screen dialog on
	// a phone, a side sheet on a wide screen. «Capturar esta pantalla» draws the screen under the panel; the draft
	// outlives the panel, so a person can close it, go to another screen and capture that one into the same report.
	import { tick, untrack } from 'svelte';
	import { api, problem } from '#lib/api/client.ts';
	import { plural, t } from '#lib/i18n.svelte.ts';
	import Icon from '#lib/nav/Icon.svelte';
	import { fadeThrough, glide, grow, vanish } from '#lib/motion.ts';
	import { focusShown } from '#lib/ui/landing.ts';
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
	// Each error next to what it answers: a send's under the header that holds «Enviar», an added image's under the add
	// buttons. Both at the form's end, a failed send said why 90 px below the screen, and «Enviar» seemed to do nothing
	// (pass 77).
	let sendError = $state('');
	let addError = $state('');
	let sendSlot = $state<HTMLElement>();
	let addSlot = $state<HTMLElement>();
	let busy = $state(false);
	let capturing = $state(false);
	const full = $derived(draft.shots.length >= MAX_SHOTS);
	// Under the add buttons, what they take or why both are off; one replacing the other fades through, as a field's hint.
	const addHint = $derived(full ? t('feedback.full') : t('feedback.shots_hint'));
	let hintShown = $state(untrack(() => addHint));
	let hintLine = $state<HTMLElement>();
	let swapping = Promise.resolve();
	$effect(() => {
		const next = addHint;
		untrack(() => {
			if (next !== hintShown)
				swapping = swapping.then(() => fadeThrough(hintLine, () => void (hintShown = addHint)));
		});
	});

	/** Brings a message just said into sight inside the panel, the focus where it was, once the rows that came with it
	 * have opened their room: measured while they grew, the message stood 700 px below the screen. */
	async function show(slot: HTMLElement | undefined) {
		await tick();
		const said = slot?.querySelector<HTMLElement>('.alert');
		const box = said?.closest('dialog');
		if (!said || !box) return;
		for (let wave = 0; wave < 4; wave++) {
			await new Promise(requestAnimationFrame);
			const running = box
				.getAnimations({ subtree: true })
				.filter(
					(a) =>
						a.effect?.getTiming().iterations !== Infinity &&
						!said.contains((a.effect as KeyframeEffect | null)?.target ?? null)
				);
			if (!running.length) break;
			await Promise.all(running.map((a) => a.finished.catch(() => undefined)));
		}
		if (said.isConnected) focusShown(said, said, false);
	}

	/** Before the panel goes: it lets the page have the focus back as it starts to leave. */
	function close() {
		sheet?.letGo();
		closePanel();
	}

	async function focusNote(key: number) {
		await tick();
		document.querySelector<HTMLTextAreaElement>(`[data-note="${key}"]`)?.focus();
	}

	// aria-disabled while it draws, not disabled: a disabled button drops the focus to the page while it works (pass 17).
	async function capture() {
		if (capturing || full) return;
		addError = '';
		capturing = true;
		try {
			const shot = await addShot(await captureScreen('dialog'));
			await focusNote(shot.key);
		} catch {
			addError = t('feedback.capture_failed');
			await show(addSlot);
		} finally {
			capturing = false;
		}
	}

	/** Every image that fits; what did not, and why, said under the buttons, the focus left on «Adjuntar». A file that is
	 * no image takes no place. */
	async function attach(files: FileList | null) {
		addError = '';
		const said: string[] = [];
		let last = 0;
		let dropped = 0;
		for (const file of files ?? []) {
			if (draft.shots.length >= MAX_SHOTS) {
				dropped += 1;
				continue;
			}
			try {
				last = (await addShot(await fromFile(file))).key;
			} catch {
				said.push(t('feedback.not_image', { name: file.name }));
			}
		}
		if (dropped) said.push(plural(dropped, 'feedback.dropped_one', 'feedback.dropped_other'));
		if (picker) picker.value = '';
		addError = said.join(' ');
		if (addError) await show(addSlot);
		else if (last) await focusNote(last);
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
		sendError = '';
		if (!draft.message.trim() && draft.shots.length === 0) {
			sendError = t('feedback.empty');
			await show(sendSlot);
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
				sendError = problem(failed, response?.status, t('feedback.what'));
			} else {
				startOver();
				notify(t('feedback.sent'));
				close();
			}
		} catch {
			sendError = problem(undefined, undefined, t('feedback.what'));
		} finally {
			busy = false;
		}
		if (sendError) await show(sendSlot);
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
		<div class="said" bind:this={sendSlot}><FormError text={sendError} /></div>
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
					disabled={full}
					aria-disabled={capturing}
					aria-describedby="feedback-add-hint"
					onclick={capture}
					><Icon name="camera" size={20} />{capturing
						? t('feedback.capturing')
						: t('feedback.capture')}</button
				>
				<button
					type="button"
					class="secondary"
					disabled={full}
					aria-describedby="feedback-add-hint"
					onclick={() => picker?.click()}
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
			<!-- What the two take, or why both are off, next to them: above the list, the reason stood far from the
			     disabled buttons (pass 77). -->
			<p class="hint" id="feedback-add-hint" bind:this={hintLine}>{hintShown}</p>
			<div class="said" bind:this={addSlot}><FormError text={addError} /></div>
		</section>

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
	/* A message's slot takes no room of its own while it is empty: the form's gap stays where the message would be. */
	.said {
		display: contents;
	}
	/* The whole screen, whatever its shape: a wide one cut to a phone's would no longer tell which screen it was (pass 77). */
	img {
		width: 96px;
		max-height: 192px;
		object-fit: contain;
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
