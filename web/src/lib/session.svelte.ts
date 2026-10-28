// Who is logged in. The layout asks once on load; the login and logout update it.
import { api, type Schemas } from '#lib/api/client.ts';

export const session = $state<{ user: Schemas['Me'] | null; checked: boolean }>({
	user: null,
	checked: false
});

// A session that ends (expired, logged out elsewhere) sends the person back to the login page: the layout
// redirects as soon as there is no user.
api.use({
	onResponse({ request, response }) {
		if (response.status === 401 && !request.url.endsWith('/api/v1/sessions')) {
			session.user = null;
		}
	}
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
