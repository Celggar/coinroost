// One notice at a time, anchored to the viewport (FD8): what an action did («Se creó BAC Corriente») or why it
// failed when the failure belongs to no field. A new one replaces the last; the next action clears it.
export type Tone = 'ok' | 'bad';
export type Notice = { id: number; text: string; tone: Tone; at: number; quiet: boolean };

export const notices = $state<{ current: Notice | null }>({ current: null });
let next = 0;

/** Shows `text`, replacing any notice on screen. */
export function notify(text: string, tone: Tone = 'ok') {
	notices.current = { id: ++next, text, tone, at: performance.now(), quiet: false };
}

/** Tells a screen reader what the screen already shows where the action happened. */
export function announce(text: string) {
	notices.current = { id: ++next, text, tone: 'ok', at: performance.now(), quiet: true };
}

/** Clears the notice, if any. */
export function dismiss() {
	notices.current = null;
}
