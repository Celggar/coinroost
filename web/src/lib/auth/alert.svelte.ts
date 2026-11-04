// The alert of a sign-in form. It keeps how to say its message, so a change of language says it again: a refusal stayed
// in the old language, and whoever switched to read it could not (pass 34).
import { untrack } from 'svelte';
import { i18n } from '#lib/i18n.svelte.ts';

export class Alert {
	text = $state('');
	/** A new `n` brings the message in again (`FormAlert`). */
	n = $state(0);
	#again: (() => string) | null = null;

	constructor() {
		// An effect, after the page has changed: a message that names a field reads its label there.
		$effect(() => {
			void i18n.locale;
			untrack(() => {
				if (this.text && this.#again) this.text = this.#again();
			});
		});
	}

	say(again: () => string) {
		this.#again = again;
		this.text = again();
		this.n += 1;
	}

	clear() {
		this.#again = null;
		this.text = '';
		this.n += 1;
	}
}
