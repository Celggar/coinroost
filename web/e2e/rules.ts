// Rules made through the API for a test: each matches its own name in capitals and takes the payee from the text.
import { expect, type Page } from '@playwright/test';

/** Makes one rule per name, in order, and returns their ids. */
export async function makeRules(page: Page, names: string[]): Promise<string[]> {
	const made: string[] = [];
	for (const name of names) {
		const response = await page.request.post('/api/v1/rules', {
			data: {
				name,
				when: { description_contains: name.toUpperCase() },
				then: { payee_from_text: true }
			}
		});
		expect(response.ok()).toBe(true);
		made.push((await response.json()).id);
	}
	return made;
}
