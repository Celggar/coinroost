// The badges' counts (P1, N6): what waits for the person under Revisar (transactions without a category, pairs to
// confirm, legs without a pair) and under Cuentas (accounts whose statements are late, as the P1 board and N6 say; one only due to check is not). They come from the
// same endpoints the pages use, refreshed on every page change and right after anything that changes them.
import { api } from '#lib/api/client.ts';
import { today } from '#lib/format.ts';

export const counts = $state({
	review: 0,
	uncategorized: 0,
	transfers: 0,
	accounts: 0
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
	// Each count on its own: a failed transfer check left Revisar without a badge while 16 transactions waited
	// (pass 7). One that fails keeps its last answer.
	if (open.data) counts.uncategorized = open.data.items.length;
	if (pairing.data) counts.transfers = pairing.data.pairs.length + pairing.data.unmatched.length;
	counts.review = counts.uncategorized + counts.transfers;
	if (coverage.data) {
		counts.accounts = coverage.data.items.filter((i) => i.delay === 'late').length;
	}
}
