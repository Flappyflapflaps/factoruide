#!/bin/sh
# The ready-made guides of the Factoruide_Ready addon: a guide for every race and class from
# level 1 (the app's default settings), so that a player follows one with no app. Plans each guide
# into routes/ready-<edition>/ (kept: run again to go on after a stop, delete it to plan again),
# writes the pack of each edition into dist/Factoruide_Ready/ and zips the addon folder.
# About 5 minutes a guide on 4 processors, 40 or so guides an edition.
#
#   scripts/ready-guides.sh                  # WoW Forever
#   scripts/ready-guides.sh forever classic  # several editions in one addon
set -eu
cd "$(dirname "$0")/.."

[ $# -gt 0 ] || set -- forever
cargo build --release --locked --quiet -p fg
fg="${CARGO_TARGET_DIR:-target}/release/fg"
mkdir -p dist
for edition in "$@"; do
    "$fg" --edition "$edition" --routes "routes/ready-$edition" ready --out dist/Factoruide_Ready
done
rm -f dist/Factoruide_Ready.zip
(cd dist && zip -qr Factoruide_Ready.zip Factoruide_Ready)
echo "wrote dist/Factoruide_Ready.zip: unzip it into the game's Interface/AddOns folder"
