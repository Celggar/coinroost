// A fresh install that lets its first account be made from the web goes straight to that, before the login paints.
import { redirect } from '@sveltejs/kit';
import { firstAccountOpen } from '#lib/session.svelte.ts';

export async function load() {
	if (await firstAccountOpen()) redirect(307, '/setup');
}
