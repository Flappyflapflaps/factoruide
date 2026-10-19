// Cloudflare Pages Function: the requests to /v1/* (the apps' contributions and problem reports)
// go on to the contribution server under this same name: the URL and its Host are kept, only the
// address is looked up under ORIGIN_HOST (a name of the same zone, proxied by Cloudflare so that
// it never gives the server's address away; a secret of the Pages project, not in this
// repository). The server sees Cloudflare's address for every sender: the sender's own goes in
// X-Factoruide-Client, with the key they share (PROXY_KEY) so that nobody else can claim one.
export async function onRequest({ request, env }) {
  // Sent back here (ORIGIN_HOST not applied): never relay a relayed request again.
  if (request.headers.has("x-factoruide-relayed") || !env.ORIGIN_HOST || !env.PROXY_KEY) {
    return new Response("relay not configured", { status: 502 });
  }
  const headers = new Headers(request.headers);
  headers.set("x-factoruide-relayed", "1");
  headers.set("x-factoruide-proxy-key", env.PROXY_KEY);
  headers.set("x-factoruide-client", request.headers.get("cf-connecting-ip") ?? "");
  return fetch(request.url, {
    method: request.method,
    headers,
    body: request.body,
    redirect: "manual",
    cf: { resolveOverride: env.ORIGIN_HOST },
  });
}
