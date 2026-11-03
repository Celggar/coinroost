// The catalogs themselves, without a browser: what every text in both languages keeps.
import { expect, test } from '@playwright/test';
import { catalogs } from './messages.ts';

// A number and its unit of measure stay on one line: «every 30 / seconds» parted at a phone's line end (pass 21).
const PARTED =
	/(\d+|\{\w+\}) (seconds?|minutes?|days?|months?|characters?|digits?|segundos?|minutos?|días?|mes(es)?|caracteres?|dígitos?)\b/;

test('a number never parts from its unit at a line end', () => {
	const parted = Object.entries(catalogs).flatMap(([language, texts]) =>
		Object.entries(texts)
			.filter(([, text]) => PARTED.test(text))
			.map(([key]) => `${language} ${key}`)
	);
	expect(parted).toEqual([]);
});
