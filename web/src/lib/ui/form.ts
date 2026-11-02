// Form checks in the page's language (FD5). Forms set `novalidate`, so the browser never shows its own bubble (in the
// browser's language, outside the page); these read the same constraints (`required`, `maxlength`, `type`,
// `pattern`, `min`/`max`) and give each invalid field its message, keyed by the field's `name`.
import { has, i18n, t } from '#lib/i18n.svelte.ts';
import { focusShown } from '#lib/ui/landing.ts';

export type Errors = Record<string, string>;

type Control = HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement;

function message(control: Control): string {
	const validity = control.validity;
	// Only spaces is empty too: the API trims and would refuse it.
	if (validity.valueMissing || (control.required && control.value.trim() === '')) {
		return t(control instanceof HTMLSelectElement ? 'form.choose' : 'form.required');
	}
	if (control instanceof HTMLSelectElement) return t('form.choose');
	if (validity.tooLong) return t('form.too_long', { max: control.maxLength });
	if (validity.tooShort) return t('form.too_short', { min: control.minLength });
	if (validity.typeMismatch && control.type === 'email') return t('form.email');
	if (validity.rangeUnderflow) return t('form.min', { min: (control as HTMLInputElement).min });
	if (validity.rangeOverflow) return t('form.max', { max: (control as HTMLInputElement).max });
	if (validity.customError) return control.validationMessage;
	// A field that mirrors an API rule names the API's own words for it (`data-format`), so both say the same.
	const format = control.dataset.format;
	if (validity.patternMismatch && format && has(format)) return t(format);
	if (control.inputMode === 'decimal') return t('form.amount');
	return t('form.format');
}

/** The form's own fields that can be checked, each with the key its error goes under: its name, else its id. */
function controls(form: HTMLFormElement): [string, Control][] {
	return [...form.elements]
		.filter((e): e is Control => 'validity' in e && (e as Control).willValidate)
		.map((control) => [control.name || control.id, control] as [string, Control])
		.filter(([key]) => key !== '');
}

/** The form's invalid fields with their messages; empty when it can be sent. */
export function check(form: HTMLFormElement): Errors {
	const errors: Errors = {};
	for (const [key, control] of controls(form)) {
		const blank = control.required && control.value.trim() === '';
		if ((control.validity.valid && !blank) || errors[key]) continue;
		errors[key] = message(control);
	}
	return errors;
}

/** Moves the focus to the first field, in the form's order, that has an error. */
export function focusFirst(form: HTMLFormElement, errors: Errors) {
	const control = controls(form).find(([key]) => errors[key])?.[1];
	if (control) focusShown(control, form);
}

/**
 * For forms that show one message (the sign-in pages): marks the invalid fields with `aria-invalid` and names every
 * one of them, so no field is marked without the message saying why. Fields with the same problem share a sentence
 * («Correo y contraseña: completa estos campos.»); '' when there is none.
 */
export function single(form: HTMLFormElement, errors: Errors): string {
	const named = new Map<string, string[]>();
	const bare: string[] = [];
	for (const [key, control] of controls(form)) {
		if (!errors[key]) {
			control.removeAttribute('aria-invalid');
			continue;
		}
		control.setAttribute('aria-invalid', 'true');
		const label = control.labels?.[0]?.firstChild?.textContent?.trim();
		if (label) named.set(errors[key], [...(named.get(errors[key]) ?? []), label]);
		else bare.push(errors[key]);
	}
	const list = new Intl.ListFormat(i18n.locale, { type: 'conjunction' });
	// After the colon the sentence goes on in lower case, and a field named after the first is a word inside it
	// (RAE, «dos puntos»; Chicago 6.63): «Correo y Contraseña: Completa…» read as a heading (pass 11).
	const sentences = [...named].map(([message, fields]) =>
		t('form.named', {
			field: list.format(fields.map((field, i) => (i ? inSentence(field) : field))),
			message: inSentence(
				fields.length > 1 && message === t('form.required') ? t('form.required_many') : message
			)
		})
	);
	return [...sentences, ...bare].join(' ');
}

/** A capitalised word in lower case, to go on inside a sentence; an acronym («TOTP») keeps its capitals. */
function inSentence(text: string): string {
	return /^\p{Lu}\p{Ll}/u.test(text)
		? text[0].toLocaleLowerCase(i18n.locale) + text.slice(1)
		: text;
}

/** The errors without the one of the field being edited: a message goes as soon as its field changes. */
/** The errors without the one under `name`: for a field the page fills by itself, which sends no event. */
export function drop(errors: Errors, name: string): Errors {
	if (!(name in errors)) return errors;
	const rest = { ...errors };
	delete rest[name];
	return rest;
}

export function without(errors: Errors, event: Event): Errors {
	const name = (event.target as Control | null)?.name;
	if (!name || !(name in errors)) return errors;
	const rest = { ...errors };
	delete rest[name];
	return rest;
}
