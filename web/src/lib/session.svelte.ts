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

// Whether the first account can still be made from the web, asked alongside the first session check so the login
// does not wait for a second round trip. Each answer is used once: the next login asks again.
let firstAccount: Promise<boolean> | undefined;

function askFirstAccount(): Promise<boolean> {
	return api.GET('/api/v1/first-account').then(({ data }) => data?.open ?? false);
}

export function firstAccountOpen(): Promise<boolean> {
	const answer = firstAccount ?? askFirstAccount();
	firstAccount = undefined;
	return answer;
}

export async function refresh(): Promise<void> {
	if (!session.checked) firstAccount ??= askFirstAccount();
	const { data } = await api.GET('/api/v1/me');
	session.user = data ?? null;
	// Someone is logged in, so the app has users: an answer asked before is stale.
	if (data) firstAccount = undefined;
	session.checked = true;
}

export async function logOut(): Promise<void> {
	await api.DELETE('/api/v1/sessions/current');
	session.user = null;
}
