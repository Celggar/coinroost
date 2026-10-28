// The API client, typed from the contract (src/lib/api/schema.d.ts is generated from spec/openapi.json).
// Same origin as the app, so the session cookie goes with every request.
import createClient from 'openapi-fetch';
import { t } from '#lib/i18n.svelte.ts';
import type { components, paths } from './schema';

export type Schemas = components['schemas'];

export const api = createClient<paths>({ baseUrl: '' });

/** The `detail` of a problem+json answer, or a plain description of what failed. */
export function problem(error: unknown, status: number | undefined, what: string): string {
	if (error && typeof error === 'object' && 'detail' in error && typeof error.detail === 'string') {
		return error.detail;
	}
	if (status === undefined) {
		return t('error.no_answer', { what });
	}
	return t('error.status', { what, status });
}

const octets = {
	bodySerializer: (body: unknown) => body as Blob,
	headers: { 'Content-Type': 'application/octet-stream' }
};

/** What importing the file would do; the account is found from the file unless given. */
export function previewFile(file: File, accountId?: string) {
	return api.POST('/api/v1/imports/preview', {
		params: { query: { file_name: file.name, account_id: accountId } },
		body: file as unknown as string,
		...octets
	});
}

/** Imports the file into the account. */
export function importFile(file: File, accountId: string) {
	return api.POST('/api/v1/accounts/{account_id}/imports', {
		params: { path: { account_id: accountId }, query: { file_name: file.name } },
		body: file as unknown as string,
		...octets
	});
}
