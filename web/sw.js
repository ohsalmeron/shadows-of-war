self.addEventListener('install', function(e) {
  self.skipWaiting();
});

self.addEventListener('activate', function(e) {
  e.waitUntil(self.clients.claim());
  // Unregister the service worker to clean up the old darkrift one
  self.registration.unregister();
});

self.addEventListener('fetch', function(event) {
  // Pass through everything, do not cache. We want fresh WASM binaries.
  event.respondWith(fetch(event.request));
});
