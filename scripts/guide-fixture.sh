#!/bin/sh
# Keep the guide output of release X.Y.Z as an addon test fixture: a small real guide (Classic
# human warrior, levels 1 to 6, the quickest planning) built with the fg CLI, written as the apps
# export it into addon/tests/guides/X.Y.Z.lua, and the list addon/tests/guides/index.lua the smoke
# test reads. Every addon must keep reading the guides of the releases before it (see the README
# there). Run again for the same release: the same file, nothing else changed; a fixture of a
# release is never rewritten.
#   scripts/guide-fixture.sh 0.14.0
set -eu
cd "$(dirname "$0")/.."

version=${1:-}
case $version in
    [0-9]*.[0-9]*.[0-9]*) ;;
    *)
        echo "usage: $0 X.Y.Z (the release whose guide output the file keeps)" >&2
        exit 2
        ;;
esac
dir=addon/tests/guides
out=$dir/$version.lua
mkdir -p "$dir"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cargo build --release --locked --quiet -p fg
fg="${CARGO_TARGET_DIR:-target}/release/fg"
"$fg" --edition classic --routes "$tmp/routes" --addon-dir "$tmp/Factoruide_Data" \
    route --race human --class warrior --to-level 6 --effort 0 >/dev/null 2>&1
"$fg" --edition classic --routes "$tmp/routes" addon --lua "$tmp/guide.lua" 2>/dev/null

latest=$(find "$dir" -name '[0-9]*.lua' ! -name "$version.lua" -exec basename {} \; | sort -V | tail -n 1)
if [ -f "$out" ]; then
    if cmp -s "$tmp/guide.lua" "$out"; then
        echo "$out is up to date"
    else
        echo "$out exists and the guide output differs now: a released fixture is never rewritten" >&2
        exit 1
    fi
elif [ -n "$latest" ] && cmp -s "$tmp/guide.lua" "$dir/$latest"; then
    echo "the guide output is the same as $latest: no fixture needed for $version"
else
    cp "$tmp/guide.lua" "$out"
    echo "wrote $out ($(wc -c <"$out") bytes)"
fi

# The list of the fixtures, oldest first.
{
    echo "-- The guides of the released apps the smoke test imports. Written by scripts/guide-fixture.sh."
    echo "return {"
    find "$dir" -name '[0-9]*.lua' -exec basename {} \; | sort -V | sed 's/.*/    "&",/'
    echo "}"
} >"$dir/index.lua"
