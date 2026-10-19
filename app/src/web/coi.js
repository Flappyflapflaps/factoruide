// Cross-origin isolation where the host does not send the headers for it (the backend's threads
// share memory, which browsers allow only to isolated pages): a service worker adds them to
// every answer, and the page loads again once under it. Loaded first by index.html.
if (typeof window === "undefined") {
  self.addEventListener("install", () => self.skipWaiting());
  self.addEventListener("activate", (e) => e.waitUntil(self.clients.claim()));
  self.addEventListener("fetch", (e) => {
    const r = e.request;
    if (r.cache === "only-if-cached" && r.mode !== "same-origin") return;
    e.respondWith(
      fetch(r).then((res) => {
        if (res.status === 0) return res;
        const headers = new Headers(res.headers);
        headers.set("Cross-Origin-Embedder-Policy", "require-corp");
        headers.set("Cross-Origin-Opener-Policy", "same-origin");
        headers.set("Cross-Origin-Resource-Policy", "same-origin");
        return new Response(res.body, { status: res.status, statusText: res.statusText, headers });
      }),
    );
  });
} else if (!window.crossOriginIsolated && window.isSecureContext && navigator.serviceWorker) {
  // Once only: a browser refusing it shows the app's message instead of reloading for ever.
  const once = () => {
    if (sessionStorage.getItem("fg-coi")) return;
    sessionStorage.setItem("fg-coi", "1");
    location.reload();
  };
  navigator.serviceWorker.addEventListener("controllerchange", once);
  navigator.serviceWorker.register(document.currentScript.src).then((reg) => {
    if (reg.active && !navigator.serviceWorker.controller) once();
  });
} else if (window.crossOriginIsolated) {
  sessionStorage.removeItem("fg-coi");
}
