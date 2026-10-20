#!/bin/sh
# The example guide both apps offer on WoW Forever ("Voir un guide d'exemple" on the Home page and
# on the empty Guides page): a human paladin from level 1 to 30, planned as the "New guide" page
# plans it with the "Thorough" effort (3) and its default settings otherwise (the class quests it
# lists, no professions, no character, guide written in English), saved as the .fgguide file the
# apps share and import: app/src/assets/example-guide.fgguide. The apps add it under the id made
# from its name (exemple-humain-paladin-1-30, see app/src/example.ts): keep the name.
#
# Run again after the game data or the planner changed (several minutes; the same guide on any
# machine, only slower with fewer processors), then check the app opens it:
#   scripts/example-guide.sh
set -eu
cd "$(dirname "$0")/.."

out=app/src/assets/example-guide.fgguide
cargo build --release --locked --quiet -p fg
fg="${CARGO_TARGET_DIR:-target}/release/fg"
"$fg" --edition forever share --race human --class paladin --from-level 1 --to-level 30 --effort 3 \
    --name "Exemple — Humain paladin 1-30" --out "$out"
