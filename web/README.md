# Factoruide in a browser

The same app as the desktop one, as a folder of static files: the guides are computed in the
browser, and kept there. Nothing needs a server but the page itself (and, to send contributions
and reports, the contribution server).

## What changes from the desktop app

The browser cannot reach the game's folder, so the addon and the app talk through texts the
player copies:

- **Characters**: in game, `/fg export` opens a window holding the character's export (its
  profile and the addon's recordings). Pasted in the **Characters** page, the character is added
  or brought up to date.
- **Guides**: the **Addon** page's "Send to the game" gives a text holding the guides your
  characters follow and the addon's settings. In game, `/fg import`, paste it, Import: the
  interface reloads with them (`/fg import clear` removes them). With Polestar (WoW Forever), it
  is a guide pack to unzip in the AddOns folder instead.
- **The addon** is installed once like any addon (its page gives a zip of its two folders);
  its generated files are empty, the guides come with `/fg import`.
- **Contributions** send the addon's recordings that came with the exports (not the game's
  caches, which the browser cannot read).
- **Data** stays in the browser (IndexedDB). Settings has a backup file to keep it or move it
  to another browser.
- Files to share (`.fgguide`, Polestar packs, configurations) are downloaded.

The texts: `FGP1:` (export) and `FGG1:` (import), then the content compressed with raw DEFLATE
and written with LibDeflate's printable alphabet (see `crates/fg-web/src/fgstring.rs` and the
addon's `Export.lua` / `Import.lua`).

## How it runs

- `crates/fg-web`: the app's backend (what `app/src-tauri` does) for the browser, built for
  `wasm32-wasip1-threads`. Its files live in memory and the page saves them in IndexedDB; the
  game databases are fetched from `data/` and read in place by SQLite.
- `app/src/web`: the page's side. `host.ts` starts the backend in a Web Worker, `worker.ts`
  gives it what it needs of WASI (clocks, randomness, sleeping) and starts its threads in Web
  Workers sharing its memory: the planner's threads are real threads. The `@tauri-apps/*`
  modules are replaced by the shims of `web/shims` (`vite build --mode web`), so the pages are
  the desktop app's, with a few changes where the game's folder was used (`IS_WEB`).
- Shared memory needs a cross-origin isolated page: the server sends
  `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp`
  (the `_headers` file does it on Netlify or Cloudflare Pages). Elsewhere (GitHub Pages...),
  `coi.js` installs a service worker that adds them and loads the page again once. The page
  must be served over HTTPS (or from localhost).

## Build

Needs Rust with the `wasm32-wasip1-threads` target, the [wasi-sdk](https://github.com/WebAssembly/wasi-sdk/releases)
(for the C parts: SQLite and zstd) and Node.js.

    rustup target add wasm32-wasip1-threads
    WASI_SDK_PATH=~/wasi-sdk-34.0-x86_64-linux scripts/build-web.sh          # into web/dist
    WASI_SDK_PATH=... scripts/build-web.sh /var/www/factoruide            # or elsewhere

The folder holds the pages, the backend (`assets/*.wasm`, about 11 MB, 3 MB compressed) and the
game data (`data/`: each game version's database, 11 to 14 MB, and its maps and minimap
tiles). Each game version's data is fetched the first time it is used. Set `FG_VERSION` to the
release's version (default: the last tag), `FG_CONTRIBUTION_SERVER` to the contribution server
(default: the server the page comes from, see below) and `VITE_FG_ADDON_URL` to the addon's page
on an addon site (shown on the Addon page).

## Serve

Any static web server does, with the headers above. Or `fg-web-server`, which sends them and
passes the contributions and reports on to the contribution server, so that the page posts to
its own server (no cross-origin request):

    cargo build --release -p fg-web-server
    target/release/fg-web-server --dir web/dist --listen 127.0.0.1:8080 \
        --upstream https://contributions.example.org

Put it behind an HTTPS proxy. It passes nothing about the player on (no address, no cookie, a
fixed user agent). Without `fg-web-server`, a site built with `FG_CONTRIBUTION_SERVER` posts to
that server directly, which must then allow the site's origin (CORS).

For a quick look: `cd app && npm run dev -- --mode web` (after a build, for the backend and the
data), then open http://localhost:5173.
