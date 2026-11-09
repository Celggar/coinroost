// The report being written: it outlives the panel, so a person can close it, go to another screen and add a screenshot
// of that one to the same report. Sent, it starts over with a new id; the API keeps a draft sent twice once.

/** The API's cap. */
export const MAX_SHOTS = 10;
export const MAX_NOTE = 500;
export const MAX_MESSAGE = 4000;

export type Kind = 'problem' | 'idea';

export type Shot = {
	/** For the list's keys and labels; never reused within a draft. */
	key: number;
	blob: Blob;
	/** `blob` as a data URL, for the thumbnail: the app's CSP takes `data:` images, not `blob:` ones. */
	url: string;
	note: string;
};

let keys = 0;

export const draft = $state({
	reportId: crypto.randomUUID(),
	kind: 'problem' as Kind,
	message: '',
	shots: [] as Shot[]
});

function dataUrl(blob: Blob): Promise<string> {
	return new Promise((resolve, reject) => {
		const reader = new FileReader();
		reader.onload = () => resolve(String(reader.result));
		reader.onerror = () => reject(reader.error);
		reader.readAsDataURL(blob);
	});
}

export async function addShot(blob: Blob): Promise<Shot> {
	draft.shots.push({ key: ++keys, blob, url: await dataUrl(blob), note: '' });
	return draft.shots[draft.shots.length - 1];
}

export function removeShot(key: number) {
	draft.shots = draft.shots.filter((s) => s.key !== key);
}

/** After a send: an empty draft with a new id. */
export function startOver() {
	draft.reportId = crypto.randomUUID();
	draft.kind = 'problem';
	draft.message = '';
	draft.shots = [];
}

/** The panel: one for the app, opened from any of its buttons, which gets the focus back when it closes. */
export const panel = $state({ open: false });
let opener: HTMLElement | null = null;

export function openPanel(from: HTMLElement | null) {
	opener = from;
	panel.open = true;
}

export function closePanel() {
	panel.open = false;
	opener?.focus();
	opener = null;
}
