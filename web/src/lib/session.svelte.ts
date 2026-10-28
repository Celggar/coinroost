// Who is logged in. The layout asks once on load; the login and logout update it.
import { api, type Schemas } from '#lib/api/client.ts';

export const session = $state<{ user: Schemas['Me'] | null; checked: boolean }>({
	user: null,
	checked: false
});

export async function refresh(): Promise<void> {
	const { data } = await api.GET('/api/v1/me');
	session.user = data ?? null;
	session.checked = true;
}

export async function logOut(): Promise<void> {
	await api.DELETE('/api/v1/sessions/current');
	session.user = null;
}
