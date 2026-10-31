// The API client, typed from the contract (src/lib/api/schema.d.ts is generated from spec/openapi.json).
// Same origin as the app, so the session cookie goes with every request.
import createClient from 'openapi-fetch';
import { has, i18n, t } from '#lib/i18n.svelte.ts';
import type { components, paths } from './schema';

export type Schemas = components['schemas'];

export const api = createClient<paths>({ baseUrl: '' });

/** What failed, in the page's language, and the field it is about when the API names one. */
export type Failure = { message: string; field?: string };

/**
 * A problem+json answer as people read it: the text of its `code`, else a sentence for its status. The API's own
 * `detail` is English, so it shows only in English and only when the code has no text yet. `field` is the body
 * member its `pointer` names, written with dots (`#/detection/markers` is `detection.markers`), which is how the
 * forms name their fields.
 */
export function failure(error: unknown, status: number | undefined, what: string): Failure {
	const body = (error && typeof error === 'object' ? error : {}) as {
		code?: unknown;
		pointer?: unknown;
		detail?: unknown;
	};
	const field =
		typeof body.pointer === 'string' && body.pointer.startsWith('#/')
			? body.pointer.slice(2).replaceAll('/', '.')
			: undefined;
	const key = `problem.${String(body.code)}`;
	// A text for the code on that field first, so a clash names what clashed (the name, the key).
	const onField = field && `${key}.${field}`;
	if (onField && has(onField)) return { message: t(onField), field };
	if (has(key)) return { message: t(key), field };
	if (status === undefined) return { message: t('error.no_answer', { what }) };
	if (i18n.locale === 'en' && typeof body.detail === 'string') {
		return { message: `${what}: ${body.detail}.`, field };
	}
	const byStatus = `problem.status_${status >= 500 ? '5xx' : status}`;
	return {
		message: has(byStatus) ? t(byStatus, { what }) : t('error.status', { what, status }),
		field
	};
}

/** Only the message of [`failure`], for answers that are not about a form field. */
export function problem(error: unknown, status: number | undefined, what: string): string {
	return failure(error, status, what).message;
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
