// The app's texts in English and Spanish. The catalogs are plain JSON in web/locales/; a key the Spanish catalog
// lacks fails the type check. Texts take `{name}` placeholders.
import en from '../../locales/en.json';
import es from '../../locales/es.json';

export type Key = keyof typeof en;
export type Locale = 'en' | 'es';

const catalogs: Record<Locale, Record<Key, string>> = { en, es };
const STORE = 'coinroost.locale';

/** The saved choice, else the first of the browser's languages the app has, else English. */
function initial(): Locale {
	try {
		const saved = localStorage.getItem(STORE);
		if (saved === 'en' || saved === 'es') return saved;
	} catch {
		// Storage can be blocked: fall back to the browser's languages.
	}
	for (const language of navigator.languages ?? [navigator.language]) {
		const code = language.toLowerCase().slice(0, 2);
		if (code === 'es' || code === 'en') return code;
	}
	return 'en';
}

export const i18n = $state({ locale: initial() });

export function setLocale(locale: Locale) {
	i18n.locale = locale;
	document.documentElement.lang = locale;
	try {
		localStorage.setItem(STORE, locale);
	} catch {
		// Not remembered, but the change still applies.
	}
}

/** `text` with its last two words held together, so where it wraps no word is left alone on the last line: CSS's
 * `text-wrap: pretty` left «… clave de / configuración» under a 360 px label (pass 22). Only a text of three words or
 * more, and only its own words: a placeholder's value can be long, a word already held to another (a hard space)
 * would make a run too long for a phone's narrow column, and so would two long words. */
export function held(text: string): string {
	const words = text.split(' ');
	if (words.length < 3) return text;
	const [before, last] = words.slice(-2);
	if (/[{} ]/.test(before + last) || before.length + last.length > 24) return text;
	return `${words.slice(0, -2).join(' ')} ${before} ${last}`;
}

const kept = new Map<string, string>();

/** The text of `key` in the current language, with `{name}` placeholders filled from `params`. */
export function t(key: Key, params?: Record<string, string | number>): string {
	const id = `${i18n.locale}:${key}`;
	let text = kept.get(id);
	if (text === undefined) {
		text = held(catalogs[i18n.locale][key]);
		kept.set(id, text);
	}
	if (!params) return text;
	return text.replace(/\{(\w+)\}/g, (whole, name: string) =>
		name in params ? String(params[name]) : whole
	);
}

/** Whether `key` is one of the app's texts, for keys built from what the server sends (a problem's code). */
export function has(key: string): key is Key {
	return key in catalogs.en;
}

/** `one` or `other` by the language's plural rule for `count`, which also fills `{count}`. */
export function plural(
	count: number,
	one: Key,
	other: Key,
	params?: Record<string, string | number>
): string {
	const rule = new Intl.PluralRules(tag()).select(count);
	return t(rule === 'one' ? one : other, { count, ...params });
}

/** The BCP 47 tag numbers and dates are formatted with. */
export function tag(): string {
	return i18n.locale === 'es' ? 'es-PA' : 'en-US';
}
