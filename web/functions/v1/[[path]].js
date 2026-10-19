// Cloudflare Pages Function: the requests to /v1/* (contributions and problem reports) are relayed
// to the contribution server. The apps installed before the web app took this name still post
// here; the newer ones post to the server directly. The server sees this relay's address for
// every sender: the sender's own goes in X-Factoruide-Client, with the key they share
// (PROXY_KEY, a secret of the Pages project) so that nobody else can claim an address.
const BACKEND = "https://api.factoruide.brouznouf.com";

export async function onRequest({ request, env }) {
  const url = new URL(request.url);
  const target = new URL(url.pathname + url.search, env.BACKEND_ORIGIN || BACKEND);
  const headers = new Headers(request.headers);
  headers.delete("x-factoruide-proxy-key");
  headers.delete("x-factoruide-client");
  if (env.PROXY_KEY) {
    headers.set("x-factoruide-proxy-key", env.PROXY_KEY);
    headers.set("x-factoruide-client", request.headers.get("cf-connecting-ip") ?? "");
  }
  return fetch(target, { method: request.method, headers, body: request.body, redirect: "manual" });
}
