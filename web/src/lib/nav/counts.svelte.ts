// The badges' counts (P1, N6): what waits for the person under Revisar (transactions without a category, pairs to
// confirm, legs without a pair) and under Cuentas (accounts whose statements are late or due). They come from the
// same endpoints the pages use, refreshed on every page change and right after anything that changes them.
import { api } from '#lib/api/client.ts';
import { today } from '#lib/format.ts';

export const counts = $state({
	review: 0,
	uncategorized: 0,
	transfers: 0,
	accounts: 0,
	late: false
});

let asked = 0;

export async function refreshCounts() {
	const mine = ++asked;
	const [open, pairing, coverage] = await Promise.all([
		api.GET('/api/v1/transactions', { params: { query: { uncategorized: true, limit: 1000 } } }),
		api.POST('/api/v1/transfers/preview'),
		api.GET('/api/v1/coverage', { params: { query: { today: today() } } })
	]);
	if (mine !== asked) return;
	if (open.data && pairing.data) {
		counts.uncategorized = open.data.items.length;
		counts.transfers = pairing.data.pairs.length + pairing.data.unmatched.length;
		counts.review = counts.uncategorized + counts.transfers;
	}
	if (coverage.data) {
		const behind = coverage.data.items.filter((i) => i.delay === 'late' || i.delay === 'warning');
		counts.accounts = behind.length;
		counts.late = behind.some((i) => i.delay === 'late');
	}
}
