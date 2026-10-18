#!/bin/sh
# Build the Linux x86_64 app for local use: the executable on its own (databases, maps and addon
# embedded; needs the system's WebKitGTK 4.1) and an AppImage carrying its libraries.
set -eu
if [ "$(uname -s)" != Linux ]; then
    echo "Run this script on Linux." >&2
    exit 1
fi
root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
# A deterministic output path, independent of the caller's Cargo configuration.
export CARGO_TARGET_DIR="$root/target"
# Local builds do not use the publisher's automatic update channel.
unset FG_RELEASE
# linuxdeploy (run by the bundler) is itself an AppImage: without FUSE, it extracts itself.
export APPIMAGE_EXTRACT_AND_RUN=1
cd "$root/app"
npm ci
npm run tauri -- build --bundles appimage
printf '\nBuilt %s\n  and the AppImage in %s\nRun either one directly.\n' \
    "$CARGO_TARGET_DIR/release/factoruide-app" "$CARGO_TARGET_DIR/release/bundle/appimage"
