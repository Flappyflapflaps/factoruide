#!/bin/sh
# Build the web app: a folder of static files any web server can serve (web/dist, or the folder
# given; see web/README.md).
#
#   WASI_SDK_PATH=~/wasi-sdk scripts/build-web.sh [out-dir]
#
# Needs the wasi-sdk (https://github.com/WebAssembly/wasi-sdk) for the C parts of the backend
# (SQLite, zstd), the Rust target wasm32-wasip1-threads and Node.js.
set -eu
cd "$(dirname "$0")/.."
out=$(mkdir -p "${1:-web/dist}" && cd "${1:-web/dist}" && pwd)

sdk=${WASI_SDK_PATH:?set WASI_SDK_PATH to the wasi-sdk folder}
version=${FG_VERSION:-$(git describe --tags --abbrev=0 2>/dev/null | sed 's/^v//' || true)}
export FG_VERSION="${version:-0.0.0}"

# The backend, for the browser: threads are Web Workers sharing one memory (up to 4 GiB), with
# a large stack for the planner's main thread. Linked as a reactor (initialized once by the
# page): a command module would run libc's exit handlers after every call and thread.
export CC_wasm32_wasip1_threads="$sdk/bin/clang"
export AR_wasm32_wasip1_threads="$sdk/bin/llvm-ar"
export CFLAGS_wasm32_wasip1_threads="--target=wasm32-wasip1-threads -pthread"
crt="$(rustc --print sysroot)/lib/rustlib/wasm32-wasip1-threads/lib/self-contained/crt1-reactor.o"
RUSTFLAGS="-C link-arg=--max-memory=4294967296 -C link-arg=-zstack-size=8388608 -C link-arg=$crt -C link-arg=--export=_initialize" \
    cargo build --release --target wasm32-wasip1-threads -p fg-web
target=${CARGO_TARGET_DIR:-target}
cp "$target/wasm32-wasip1-threads/release/fg_web.wasm" app/src/web/fg_web.wasm

# The pages.
(cd app && [ -d node_modules ] || npm ci)
(cd app && npx tsc -b && FG_WEB_OUT="$out" npx vite build --mode web)

# The game data, the maps and the minimap tiles.
node scripts/web-data.mjs "$out"
echo "web app in $out"
