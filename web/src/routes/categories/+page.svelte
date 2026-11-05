<script lang="ts">
	// The household's categories, a tab of Reglas: until here they showed only inside the selectors. Renaming and
	// deleting need endpoints of their own and are not here yet.
	import { onMount, tick } from 'svelte';
	import { api, problem, type Schemas } from '#lib/api/client.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import { arrive, grow, placeholder, swapIn, vanish } from '#lib/motion.ts';
	import CategoryForm from '#lib/rules/CategoryForm.svelte';
	import { holds } from '#lib/ui/CategoryOptions.svelte';
	import { retry } from '#lib/ui/focus.ts';
	import { firstRoom, follow } from '#lib/ui/hold-room.ts';
	import { focusShown } from '#lib/ui/landing.ts';

	let categories = $state<Schemas['Category'][]>([]);
	let loadError = $state('');
	let loaded = $state(false);
	let list = $state<HTMLElement>();

	async function load() {
		loadError = '';
		const { data, error, response } = await api.GET('/api/v1/categories');
		if (!data) {
			loadError = problem(error, response?.status, t('categories.failed'));
			return;
		}
		categories = data.items.toSorted((a, b) => a.name.localeCompare(b.name));
		loaded = true;
	}

	/** A new row opens its room, and the first one the list's: taken at once, the form with the focus fell 48 px (pass 52). */
	async function created() {
		const first = !categories.length;
		const before = list?.offsetHeight ?? 0;
		await load();
		await tick();
		if (first) firstRoom(list, before);
	}

	// A link to the form (from an uncategorized queue with no category to choose) lands in its name field: the form only
	// exists once the page has loaded, too late for the browser's own jump to the anchor.
	let fromQueue = $state(false);
	onMount(async () => {
		fromQueue = location.hash === '#new-category';
		await load();
		if (fromQueue) {
			await tick();
			// What stood in for the page leaves the flow as it fades, and the bar settles its height: measured before
			// that, the landing aimed 32 px off and left a label across the bar (pass 12).
			await Promise.all(
				document
					.getAnimations()
					.filter((a) => a.effect?.getTiming().iterations !== Infinity)
					.map((a) => a.finished.catch(() => undefined))
			);
			const form = document.getElementById('new-category');
			const name = form?.querySelector('input');
			// The form from its title, its name field focused and its button in view.
			if (form && name) focusShown(name, form);
		}
	});
</script>

<svelte:head>
	<title>{t('app.title', { page: t('categories.title') })}</title>
</svelte:head>

<h1>{t('categories.title')}</h1>

{#if loadError}
	<p class="alert" role="alert" in:arrive out:placeholder>
		{loadError}
		<button class="secondary" onclick={(event) => retry(event.currentTarget, load)}
			>{t('app.try_again')}</button
		>
	</p>
{:else if loaded}
	<div class="stack" in:arrive>
		<div class="in-place" bind:this={list}>
			{#if categories.length}
				<ul class="card list" aria-label={t('categories.title')} in:swapIn>
					{#each categories as category (category.id)}
						<li in:grow onintrostart={follow}>
							<span>{category.name}</span>
							<span class="hint">{t(holds[category.kind])}</span>
						</li>
					{/each}
				</ul>
			{:else}
				<p out:vanish>{t('categories.none_yet')}</p>
			{/if}
		</div>
		<CategoryForm oncreated={created} back={fromQueue} />
	</div>
{:else}
	<p class="hint loading" out:placeholder>{t('app.loading')}</p>
{/if}

<style>
	.list {
		list-style: none;
		margin: 0;
		padding: 0;
		gap: 0;
	}
	li {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2xs) var(--space-md);
		min-height: 48px;
		padding: var(--space-sm) var(--space-lg);
		overflow-wrap: anywhere;
	}
	li + li {
		border-top: 1px solid var(--border);
	}
</style>
