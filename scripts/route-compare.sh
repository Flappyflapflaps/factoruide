#!/bin/sh
# Did a planner or data change move the routes? A panel of starts planned before and after.
#
#   scripts/route-compare.sh base    on the clean tree, before the edits: plan the panel into the cache
#   scripts/route-compare.sh         after the edits: plan it again and diff each guide against the base
#
# The panel is FG_PANEL ("edition:race:class ..."), planned at effort 0 up to FG_TO_LEVEL (default 30;
# 60 for the whole leveling). Only the planner and the data change the routes: skip this for the
# addon and the interface. The cache is $FG_COMPARE (default /tmp/fg-compare); `fg` is
# target/release/fg, built here. Same steps and times means no change at all.
set -eu
cd "$(dirname "$0")/.."

root=${FG_COMPARE:-/tmp/fg-compare}
panel=${FG_PANEL:-"forever:orc:warrior classic:orc:warlock forever:human:mage tbc:bloodelf:paladin"}
to=${FG_TO_LEVEL:-30}
fg=target/release/fg

plan() { # DIR
    mkdir -p "$1"
    for spec in $panel; do
        IFS=: read -r edition race class <<EOF
$spec
EOF
        "$fg" route --edition "$edition" --race "$race" --class "$class" --to-level "$to" --effort 0 \
            --routes "$1" --addon-dir "$1/addon" --name "$edition-$race-$class" >"$1/$edition-$race-$class.log" 2>&1 \
            || { echo "$spec: planning failed, see $1/$edition-$race-$class.log" >&2; exit 1; }
        printf '%s: %s\n' "$spec" "$(grep -m1 ' in [0-9]*h[0-9]* ' "$1/$edition-$race-$class.log" | sed 's/^.*level/level/')"
    done
}

cargo build --release --bin fg 2>&1 | tail -1
case ${1:-compare} in
base)
    rm -rf "$root/base"
    git diff --quiet || echo "warning: the tree is not clean, the base is not a commit's" >&2
    plan "$root/base"
    ;;
compare)
    [ -d "$root/base" ] || { echo "no base: run '$0 base' on the clean tree first" >&2; exit 1; }
    rm -rf "$root/new"
    plan "$root/new"
    for spec in $panel; do
        IFS=: read -r edition race class <<EOF
$spec
EOF
        name=$edition-$race-$class
        echo "== $spec"
        "$fg" diff --edition "$edition" --routes "$root/base" "$root/base/$name.json" "$root/new/$name.json" | head -${FG_DIFF_LINES:-12}
    done
    ;;
*)
    sed -n '2,11p' "$0" | cut -c3- >&2
    exit 2
    ;;
esac
