// A fresh install that lets its first account be made from the web goes straight to that, before the login paints.
import { redirect } from '@sveltejs/kit';
import { api } from '#lib/api/client.ts';

export async function load() {
	const { data } = await api.GET('/api/v1/first-account');
	if (data?.open) redirect(307, '/setup');
}
