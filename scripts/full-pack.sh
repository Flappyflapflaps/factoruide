#!/bin/sh
# The full pack for a player's own install: Factoruide_Profile and Factoruide_Guide as published
# (their files in git at HEAD, the empty Config.lua, Routes.lua, Services.lua and Sent.lua
# included, so the game finds every file their .toc lists) and the ready-made guides
# (dist/Factoruide_Ready, from scripts/ready-guides.sh). The empty files hold nothing of the
# player's: the guides are kept in the saved variables and in Factoruide_Data.
#
#   scripts/full-pack.sh                  # dist/Factoruide_Full.zip
#   scripts/full-pack.sh out/Pack.zip
set -eu
cd "$(dirname "$0")/.."

out=${1:-dist/Factoruide_Full.zip}
[ -f dist/Factoruide_Ready/Factoruide_Ready.toc ] || {
    echo "no dist/Factoruide_Ready: run scripts/ready-guides.sh first" >&2
    exit 1
}
[ -z "$(git status --porcelain addon/Factoruide_Profile addon/Factoruide_Guide)" ] ||
    echo "note: uncommitted addon changes are not packed" >&2

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
git archive --format=tar HEAD addon/Factoruide_Profile addon/Factoruide_Guide | tar -x -C "$tmp"
for f in Factoruide_Guide/Config.lua Factoruide_Guide/Routes.lua Factoruide_Guide/Services.lua Factoruide_Profile/Sent.lua; do
    [ -f "$tmp/addon/$f" ] || {
        echo "missing $f" >&2
        exit 1
    }
done
cp -R dist/Factoruide_Ready "$tmp/addon/"
mkdir -p "$(dirname "$out")"
out=$(cd "$(dirname "$out")" && pwd)/$(basename "$out")
rm -f "$out"
(cd "$tmp/addon" && zip -qr -9 "$out" Factoruide_Profile Factoruide_Guide Factoruide_Ready)
echo "wrote $out: unzip it into the game's Interface/AddOns folder"
