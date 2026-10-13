#!/bin/sh
# Build and ad-hoc sign a macOS app for local use (no developer certificate required).
set -eu
if [ "$(uname -s)" != Darwin ]; then
    echo "Run this script on macOS." >&2
    exit 1
fi
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
# A deterministic output path, independent of the caller's Cargo configuration.
export CARGO_TARGET_DIR="$root/target"
# Local builds do not use the publisher's automatic update channel.
unset FG_RELEASE
cd "$root/app"
npm ci
npm run tauri -- build --bundles app
bundle="$CARGO_TARGET_DIR/release/bundle/macos/Factoruide.app"
codesign --force --deep --sign - "$bundle"
codesign --verify --deep --strict "$bundle"
printf '\nBuilt %s\nOpen it with: open "%s"\n' "$bundle" "$bundle"
