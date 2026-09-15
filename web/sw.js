const scope = new URL(self.registration.scope);
const prefix = `flip-seven:${scope.pathname}:`;
const cacheName = prefix + "__BUILD_HASH__";
const assets = __PRECACHE__;
const indexURL = new URL("index.html", scope).href;

self.addEventListener("install", event => {
  event.waitUntil(caches.open(cacheName).then(cache => cache.addAll(assets)));
  // Wait for open games to close before activating a new release.
});

self.addEventListener("activate", event => {
  event.waitUntil((async () => {
    for (const key of await caches.keys()) {
      if (key.startsWith(prefix) && key !== cacheName) await caches.delete(key);
    }
    await self.clients.claim();
  })());
});

self.addEventListener("fetch", event => {
  const url = new URL(event.request.url);
  if (event.request.method !== "GET" || url.origin !== scope.origin || !url.pathname.startsWith(scope.pathname)) return;
  event.respondWith((async () => {
    const cache = await caches.open(cacheName);
    const isAppNavigation = event.request.mode === "navigate"
      && [scope.pathname, new URL(indexURL).pathname].includes(url.pathname);
    return (await cache.match(isAppNavigation ? indexURL : event.request)) || fetch(event.request);
  })());
});
