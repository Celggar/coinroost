// The code an authenticator app shows for a secret (RFC 6238: HMAC-SHA-1, 6 digits, 30-second steps).
import { createHmac } from 'node:crypto';

const ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';

function fromBase32(text: string): Buffer {
	let bits = 0;
	let value = 0;
	const bytes: number[] = [];
	for (const char of text.replace(/=+$/, '').toUpperCase()) {
		const index = ALPHABET.indexOf(char);
		if (index < 0) throw new Error(`not base32: ${char}`);
		value = (value << 5) | index;
		bits += 5;
		if (bits >= 8) {
			bytes.push((value >>> (bits - 8)) & 0xff);
			bits -= 8;
		}
	}
	return Buffer.from(bytes);
}

export function totp(secret: string, now = Date.now()): string {
	const counter = Buffer.alloc(8);
	counter.writeBigUInt64BE(BigInt(Math.floor(now / 1000 / 30)));
	const mac = createHmac('sha1', fromBase32(secret)).update(counter).digest();
	const offset = mac[mac.length - 1] & 0xf;
	return String((mac.readUInt32BE(offset) & 0x7fffffff) % 1_000_000).padStart(6, '0');
}
