<script lang="ts">
	// Nothing imported yet: the step that comes first, said the same on every page that lists transactions. With no
	// account, or none that reads a bank file, Import would only send people on to Accounts (pass 6).
	import type { Schemas } from '#lib/api/client.ts';
	import { t } from '#lib/i18n.svelte.ts';
	import EmptyState from '#lib/ui/EmptyState.svelte';

	let { accounts }: { accounts: Schemas['Account'][] } = $props();
</script>

{#if accounts.length === 0}
	<EmptyState
		icon="bank"
		text={t('dashboard.no_accounts')}
		href="/accounts"
		action={t('dashboard.to_accounts')}
	/>
{:else if !accounts.some((a) => a.profile)}
	<EmptyState
		icon="bank"
		text={t('import.no_bank_file')}
		href="/accounts"
		action={t('file.open_accounts')}
	/>
{:else}
	<EmptyState
		icon="import"
		text={t('uncategorized.none_yet')}
		href="/import"
		action={t('uncategorized.import_first')}
	/>
{/if}
