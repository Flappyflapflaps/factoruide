# Factoruide in a browser

The same app as the desktop one, as a folder of static files: the guides are computed in the
browser, and kept there. It sends nothing anywhere: any static host serves it (GitHub Pages,
Cloudflare Pages...).

## What changes from the desktop app

The browser cannot reach the game's folder, so the addon and the app talk through texts the
player copies:

- **Characters**: in game, the addon's options (`/fg`), **Export my character**, gives the
  character's export (its profile). Pasted in **New guide** ("Start from a character", or opened
  from a text file), the guide starts from where the character stands.
- **Guides**: on the Guides page, each guide's **Export** → Addon gives a text: in game, `/fg`,
  **Import a guide…**, paste it, Import. The guide joins those of the addon (one of the same name
  is replaced; an older one asks first), and the guides of the addon's options choose the one each
  character follows (by itself when it is the only one of its race and class). **Export all**
  downloads every guide at once as the `Factoruide_Data` folder to unzip in the AddOns folder,
  and on WoW Forever as a Polestar guide pack.
- **The addon** is installed and updated from CurseForge like any addon, and set up in game
  (`/fg` opens its page in the game's options: AddOns → Factoruide). Neither app installs or
  configures it (the desktop app writes its guides in a data addon of its own, `Factoruide_Data`).
- **Data** stays in the browser (IndexedDB). The menu's Save / Restore download a backup file and
  read it back (to keep it, or move it to another browser); the language and the theme are there
  too.
- No contributions and no reports from the browser: problems go to the project's GitHub issues.
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

## Memory and speed

A 1-60 guide (Classic, "Normal" computing time) takes about 190 s in Chrome (Linux or Windows),
about 1.3 times the desktop app, and gives the same guide. The backend reserves its memory
when it starts (up to 3.75 GiB, before any thread runs: a thread copying memory that another one
just grew traps in the browser). Only what is used takes RAM (about 2 GB for that guide), but
Windows counts the whole reservation in the memory committed by the browser.

## Build

Needs Rust with the `wasm32-wasip1-threads` target, the [wasi-sdk](https://github.com/WebAssembly/wasi-sdk/releases)
(for the C parts: SQLite and zstd) and Node.js.

    rustup target add wasm32-wasip1-threads
    WASI_SDK_PATH=~/wasi-sdk-34.0-x86_64-linux scripts/build-web.sh          # into web/dist
    WASI_SDK_PATH=... scripts/build-web.sh /var/www/factoruide            # or elsewhere

The folder holds the pages, the backend (`assets/*.wasm`, about 11 MB, 3 MB compressed) and the
game data (`data/`: each game version's database, 11 to 14 MB, and its maps and minimap
tiles). Each game version's data is fetched the first time it is used. Set `FG_VERSION` to the
release's version (default: the last tag) and `VITE_FG_ADDON_URL` to the addon's page on
CurseForge (default: a search for it there).

## Serve

Any static web server does, with the headers above (or without them, through `coi.js`). For a
quick look: `cd app && npm run dev -- --mode web` (after a build, for the backend and the data),
then open http://localhost:5173.

## Deployment

Each release (`scripts/release.sh`) builds the web app from its tag and deploys it to Cloudflare
Pages (`.github/workflows/release.yml`, job `web`), at `factoruide.brouznouf.com`. The repository
needs the secrets `CLOUDFLARE_API_TOKEN` (allowed to edit Cloudflare Pages),
`CLOUDFLARE_ACCOUNT_ID` and `FG_PROXY_KEY`, and optionally the variable `CLOUDFLARE_PAGES_PROJECT`
(default `factoruide`; the job creates the project the first time). The `_headers` file gives
the page its cross-origin isolation there. The free plan's limits (20,000 files, 25 MiB a file)
leave room: about 8,200 files, the largest 14 MB.

That name was the contribution server's: the desktop apps released before still post their
contributions and reports to `/v1/*` there. `functions/v1/[[path]].js` (a Pages Function, the
only path that runs code: the rest is served as files) relays them to the server, now at
`api.factoruide.brouznouf.com`, where newer apps post directly. It tells the server each
sender's address with the key both share (`FG_PROXY_KEY`, set as the project's `PROXY_KEY`; see
the contribution server's README).
