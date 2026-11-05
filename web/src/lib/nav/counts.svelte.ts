// The badges' counts (P1, N6): what waits for the person under Revisar (transactions without a category, pairs to
// confirm, legs without a pair) and under Cuentas (accounts whose statements are late, as the P1 board and N6 say; one only due to check is not). They come from the
// same endpoints the pages use, refreshed on every page change and right after anything that changes them.
import { untrack } from 'svelte';
import { api } from '#lib/api/client.ts';
import { today } from '#lib/format.ts';

export const counts = $state({
	review: 0,
	uncategorized: 0,
	transfers: 0,
	accounts: 0
});

/** What the summary's «Por revisar» card says about those counts (P3), from the same answers, so it never disagrees
 * with the badges or with the pages it links to. */
export const pending = $state({
	ready: false,
	/** Money that left in the uncategorized transactions, per currency (minor units, positive). */
	spending: [] as { currency: string; minor: number }[],
	/** The late accounts, with their latest data. */
	late: [] as { name: string; latest: string | null }[],
	/** The accounts with no data at all. */
	unread: [] as string[],
	/** Some account has data: until one does, nothing can be up to date, and the card has nothing true to say. */
	started: false,
	pairs: 0,
	unmatched: 0,
	/** The first answers failed: the summary shows without the card rather than wait for ever. */
	failed: false
});

let asked = 0;
// The last answers, whole, to tell that something moved.
let said = '';
const away = $state({ moved: 0 });

/** Asks again; true when all answered and what they say moved since the last time. */
export async function refreshCounts(): Promise<boolean> {
	const mine = ++asked;
	const [open, pairing, coverage] = await Promise.all([
		api.GET('/api/v1/transactions', { params: { query: { uncategorized: true, limit: 1000 } } }),
		api.POST('/api/v1/transfers/preview'),
		api.GET('/api/v1/coverage', { params: { query: { today: today() } } })
	]);
	if (mine !== asked) return false;
	// The same answers write nothing: a tab coming back to no news moves nothing, not even a row's text.
	const now = JSON.stringify([open.data, pairing.data, coverage.data]);
	if (now === said) return false;
	said = now;
	// Each count on its own: a failed transfer check left Revisar without a badge while 16 transactions waited
	// (pass 7). One that fails keeps its last answer.
	if (open.data) {
		counts.uncategorized = open.data.items.length;
		const out = new Map<string, number>();
		for (const leg of open.data.items.flatMap((x) => x.legs)) {
			if (leg.amount_minor < 0)
				out.set(leg.currency, (out.get(leg.currency) ?? 0) - leg.amount_minor);
		}
		pending.spending = [...out].map(([currency, minor]) => ({ currency, minor }));
	}
	if (pairing.data) {
		counts.transfers = pairing.data.pairs.length + pairing.data.unmatched.length;
		pending.pairs = pairing.data.pairs.length;
		pending.unmatched = pairing.data.unmatched.length;
	}
	counts.review = counts.uncategorized + counts.transfers;
	if (coverage.data) {
		const late = coverage.data.items.filter((i) => i.delay === 'late');
		counts.accounts = late.length;
		pending.late = late.map((i) => ({ name: i.name, latest: i.latest ?? null }));
		pending.unread = coverage.data.items.filter((i) => !i.latest).map((i) => i.name);
		pending.started = pending.unread.length < coverage.data.items.length;
	}
	// Once all three answered, the card shows; a later failed refresh keeps the last numbers.
	if (open.data && pairing.data && coverage.data) {
		pending.ready = true;
		return true;
	}
	if (!pending.ready) pending.failed = true;
	return false;
}

/**
 * Back on a tab left open: the counts are asked again, and a page that shows what they read loads again if it moved
 * ([`onMoved`]). Sorted in another tab, the summary went on saying «16 transacciones sin categoría» (pass 37).
 */
export async function cameBack() {
	if (document.visibilityState === 'visible' && (await refreshCounts())) away.moved += 1;
}

/** For a page that shows what the counts read: loads again when it moved while the person was away. */
export function onMoved(load: () => void) {
	let seen = away.moved;
	$effect(() => {
		if (away.moved === seen) return;
		seen = away.moved;
		untrack(load);
	});
}

/** For a list that moved: it opens afresh, as a page does. Changed in place, with no action to follow, rows left with the
 * focus in them, the page's end pulled back 174 px and new rows came in one frame (pass 38). */
export const reloadOnMoved = () => onMoved(() => location.reload());
