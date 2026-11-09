<script lang="ts">
	// «Comentarios»: what the household told about the app, newest first, each report with its screenshots and their
	// notes, and whether it is open or resolved.
	import { onMount } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { t, tag } from '#lib/i18n.svelte.ts';
	import { openPanel, panel } from '#lib/feedback/draft.svelte.ts';
	import { arrive, placeholder } from '#lib/motion.ts';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import { retry } from '#lib/ui/focus.ts';
	import { notify } from '#lib/ui/notices.svelte.ts';

	let reports = $state<Schemas['Report'][] | null>(null);
	let error = $state('');
	let busy = $state<string | null>(null);

	async function load() {
		error = '';
		const { data, error: failed, response } = await api.GET('/api/v1/feedback');
		if (!data) {
			reports = null;
			error = problem(failed, response?.status, t('feedback.what'));
			return;
		}
		reports = data.items;
	}

	async function toggle(report: Schemas['Report']) {
		busy = report.id;
		const resolved = !report.resolved_at;
		const {
			data,
			error: failed,
			response
		} = await api.PATCH('/api/v1/feedback/{feedback_id}', {
			params: { path: { feedback_id: report.id } },
			body: { resolved }
		});
		busy = null;
		if (!data) {
			notify(problem(failed, response?.status, t('feedback.what')), 'bad');
			return;
		}
		reports = (reports ?? []).map((r) => (r.id === data.id ? data : r));
		notify(t(resolved ? 'feedback.resolved' : 'feedback.reopened'));
	}

	// As the app writes a day («9 nov 2026»), with the time it was sent.
	const when = (iso: string) =>
		new Intl.DateTimeFormat(tag(), {
			day: 'numeric',
			month: 'short',
			year: 'numeric',
			hour: 'numeric',
			minute: '2-digit'
		}).format(new Date(iso));

	onMount(load);
	// A report sent from this page shows up once the panel closes.
	let wasOpen = false;
	$effect(() => {
		if (wasOpen && !panel.open) load();
		wasOpen = panel.open;
	});
</script>

<svelte:head>
	<title>{t('app.title', { page: t('feedback.title') })}</title>
</svelte:head>

<h1>{t('feedback.title')}</h1>

{#if error}
	<p class="alert" role="alert" in:arrive out:placeholder>
		{error}
		<button class="secondary" onclick={(event) => retry(event.currentTarget, load)}
			>{t('app.try_again')}</button
		>
	</p>
{:else if reports}
	<div class="list" in:arrive>
		{#each reports as report (report.id)}
			<article class="card report">
				<header>
					<span class="chip {report.resolved_at ? 'ok' : 'warn'}"
						>{t(report.resolved_at ? 'feedback.state_resolved' : 'feedback.state_open')}</span
					>
					<h2>{t(report.kind === 'idea' ? 'feedback.kind_idea' : 'feedback.kind_problem')}</h2>
					<time datetime={report.created_at}>{when(report.created_at)}</time>
				</header>
				<p class="message" class:hint={!report.message}>
					{report.message || t('feedback.no_message')}
				</p>
				<p class="hint screen">{t('feedback.screen', { path: report.context.path })}</p>
				{#if report.shots.length}
					<ol class="shots">
						{#each report.shots as shot, i (shot.position)}
							<li>
								<a href={shot.url} target="_blank" rel="noopener"
									><img
										src={shot.url}
										loading="lazy"
										alt={t('feedback.shot_alt', { n: i + 1 })}
									/></a
								>
								<p class:hint={!shot.note}>{shot.note || t('feedback.no_note')}</p>
							</li>
						{/each}
					</ol>
				{/if}
				<button
					class="secondary"
					aria-disabled={busy === report.id}
					onclick={() => busy || toggle(report)}
					>{t(report.resolved_at ? 'feedback.reopen' : 'feedback.resolve')}</button
				>
			</article>
		{:else}
			<EmptyState
				icon="feedback"
				text={t('feedback.none')}
				action={t('feedback.button_label')}
				onclick={() => openPanel(document.activeElement as HTMLElement | null)}
			/>
		{/each}
	</div>
{/if}

<style>
	.list {
		display: grid;
		gap: var(--space-lg);
		max-width: 720px;
	}
	.report {
		display: grid;
		gap: var(--space-sm);
		justify-items: start;
	}
	header {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-sm) var(--space-md);
		width: 100%;
	}
	h2 {
		margin: 0;
		font-size: var(--text-lg);
	}
	time {
		margin-left: auto;
		color: var(--muted);
		font-size: var(--text-sm);
	}
	p {
		margin: 0;
	}
	.message {
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
	.screen {
		overflow-wrap: anywhere;
	}
	.shots {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
		gap: var(--space-md);
		width: 100%;
	}
	.shots li {
		display: grid;
		gap: var(--space-xs);
		align-content: start;
	}
	.shots a {
		display: block;
		border-radius: var(--radius-field);
	}
	img {
		display: block;
		width: 100%;
		aspect-ratio: 3 / 4;
		object-fit: cover;
		object-position: top;
		border: 1px solid var(--border);
		border-radius: var(--radius-field);
		background: var(--bg);
	}
	.shots p {
		font-size: var(--text-sm);
		overflow-wrap: anywhere;
	}
</style>
