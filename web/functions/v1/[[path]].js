// Cloudflare Pages Function: the requests to /v1/* (the apps' contributions and problem reports)
// go on to the contribution server through ORIGIN_HOST, a name of the zone proxied by Cloudflare
// so that it never gives the server's address away (a secret of the Pages project, not in this
// repository). An Origin Rule on that name gives the server its usual Host
// (factoruide.brouznouf.com). The server sees Cloudflare's address for every sender: the sender's
// own goes in X-Factoruide-Client, with the key they share (PROXY_KEY) so that nobody else can
// claim one.

export async function onRequest({ request, env }) {
  // Back here (ORIGIN_HOST leading to this site): never relay a relayed request again.
  if (request.headers.has("x-factoruide-relayed")) {
    return new Response("relay loop: ORIGIN_HOST leads back to the web app", { status: 508 });
  }
  if (!env.ORIGIN_HOST || !env.PROXY_KEY) {
    return new Response("relay not configured (ORIGIN_HOST, PROXY_KEY)", { status: 502 });
  }
  const url = new URL(request.url);
  url.hostname = env.ORIGIN_HOST;
  url.port = "";
  const headers = new Headers(request.headers);
  headers.set("x-factoruide-relayed", "1");
  headers.set("x-factoruide-proxy-key", env.PROXY_KEY);
  headers.set("x-factoruide-client", request.headers.get("cf-connecting-ip") ?? "");
  return fetch(url, { method: request.method, headers, body: request.body, redirect: "manual" });
}
