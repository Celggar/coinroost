/**
 * The text a new rule starts from: the description less what ends it with a digit (a branch, a date, a reference:
 * «GASBILL 032026»). The rest stays whole, however long: a rule too narrow misses a line and can be shortened in its
 * field, while one cut to its first words («YAPPY A») would take every payment through that channel.
 */
export function ruleText(description: string): string {
	const words = description.trim().split(/\s+/);
	while (words.length > 1 && /\d/.test(words[words.length - 1])) words.pop();
	return words.join(' ');
}
