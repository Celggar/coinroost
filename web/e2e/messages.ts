// The app's texts, read from its catalogs so the tests follow the wording instead of copying it.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import type catalog from '../locales/en.json';

export type Key = keyof typeof catalog;
type Params = Record<string, string | number>;

const load = (language: string): Record<Key, string> =>
	JSON.parse(
		readFileSync(fileURLToPath(new URL(`../locales/${language}.json`, import.meta.url)), 'utf8')
	);
const es = load('es');
const en = load('en');

const fill = (text: string, params: Params = {}) =>
	text.replace(/\{(\w+)\}/g, (whole, name: string) =>
		name in params ? String(params[name]) : whole
	);

/** The Spanish text, the language the run's browser asks for. */
export const say = (key: Key, params?: Params) => fill(es[key], params);

/** The English text, after switching the language. */
export const english = (key: Key, params?: Params) => fill(en[key], params);
