// What makes the app installable: the browser offers to install a site whose service worker answers its requests. It keeps
// the app's own files, so the installed app opens at once and, offline, still opens. The API never goes through a cache:
// the ledger and the session are always the server's answer.
import { version } from '$app/env';
import { assets, immutable } from '$app/manifest';
import { self as worker } from '$app/service-worker';

const CACHE = `coinroost-${version}`;
// The single-page shell every route falls back to: what a page opened offline gets.
const SHELL = '/200.html';
// The manifest names them from the base path, without its slash; a request's path starts with one.
const ASSETS = new Set([...immutable, ...assets].map((file) => `/${file.path}`));

worker.addEventListener('install', (event) => {
	event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll([...ASSETS, SHELL])));
});

worker.addEventListener('activate', (event) => {
	event.waitUntil(
		caches
			.keys()
			.then((keys) =>
				Promise.all(keys.filter((key) => key !== CACHE).map((key) => caches.delete(key)))
			)
	);
});

worker.addEventListener('fetch', (event) => {
	const request = event.request;
	const url = new URL(request.url);
	if (request.method !== 'GET' || url.origin !== location.origin) return;
	if (url.pathname.startsWith('/api/') || url.pathname === '/mcp') return;
	if (request.mode === 'navigate') {
		// The network first, so a deploy shows at once; the kept shell only when there is none.
		event.respondWith(
			fetch(request).catch(async () => (await caches.match(SHELL)) ?? Response.error())
		);
		return;
	}
	if (ASSETS.has(url.pathname)) {
		event.respondWith(caches.match(url.pathname).then((kept) => kept ?? fetch(request)));
	}
});
