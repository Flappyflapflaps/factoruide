#!/bin/bash
# The group guide scenario: characters made by planning solo routes (their profiles are written
# where each route leaves the character, `--profile-out`), then group guides from them.
#
#   scripts/group-scenario.sh            profiles, the rules, a 3 players guide, a wide gap (~1 min)
#   scripts/group-scenario.sh sync       also the levels kept together or not, at effort 1 (~1 min more)
#
# Your own characters instead: export each one in game (the addon's options, Export my character),
# turn the FGP1: text into a profile JSON (`luajit addon/tests/fgstring.lua decode <file>`, the
# "profile" field is the JSON) and give them to `fg route --start A.json --mate B.json ...`.
# Work files in $FG_GROUP (default /tmp/fg-group).
set -eu
cd "$(dirname "$0")/.."
fg=target/release/fg
out=${FG_GROUP:-/tmp/fg-group}
mkdir -p "$out"
cargo build --release --bin fg 2>&1 | tail -1

# A solo route to LEVEL; leaves the character as a profile.
character() { # NAME RACE CLASS LEVEL
    [ -f "$out/$1.json" ] ||
        "$fg" route --race "$2" --class "$3" --to-level "$4" --effort 0 --routes "$out/solo" \
            --addon-dir "$out/addon" --name "solo-$1" --profile-out "$out/$1.json" >/dev/null 2>&1
}
character warrior14 human warrior 14
character priest12 human priest 12
character mage11 human mage 11
character rogue8 human rogue 8
character rogue24 human rogue 24
character orc12 orc warrior 12

# A group guide: the first profile is --start, the others --mate; the rest goes to fg route.
group() { # NAME LEVEL EFFORT PROFILE... [-- fg options]
    local name=$1 to=$2 effort=$3 first=$4 mates=() extra=()
    shift 4
    while [ $# -gt 0 ] && [ "$1" != "--" ]; do mates+=(--mate "$out/$1.json"); shift; done
    [ $# -gt 0 ] && shift && extra=("$@")
    "$fg" route --race human --class warrior --start "$out/$first.json" "${mates[@]}" --to-level "$to" \
        --effort "$effort" --routes "$out/guides" --addon-dir "$out/addon" --name "$name" \
        "${extra[@]}" 2>&1 | tee "$out/$name.log" | tail -1 >/dev/null || true
}
summary() { # NAME
    printf '%-14s %s | %s\n' "$1" "$(grep -m1 "^$1:" "$out/$1.log" | sed 's/.*level/level/')" \
        "$(grep 'group levels' "$out/$1.log" | sed 's/^ *group levels //')"
}
refuses() { # WHAT EXPECTED PROFILE...
    local what=$1 expected=$2 first=$3
    shift 3
    local mates=() p
    for p in "$@"; do mates+=(--mate "$out/$p.json"); done
    if msg=$("$fg" route --race human --class warrior --start "$out/$first.json" "${mates[@]}" --to-level 25 \
        --effort 0 --routes "$out/guides" --addon-dir "$out/addon" --name refused 2>&1) ; then
        echo "FAIL  $what: a guide was made"
    elif printf '%s' "$msg" | grep -q "$expected"; then
        echo "ok    $what: $(printf '%s' "$msg" | tail -1)"
    else
        echo "FAIL  $what: $(printf '%s' "$msg" | tail -1)"
    fi
}

echo "== rules"
refuses "a player under level 10" "must be level 10" warrior14 rogue8
refuses "two factions" "same faction" warrior14 orc12
refuses "six players" "5 players at most" warrior14 priest12 mage11 priest12 mage11 priest12

echo "== 3 players (14, 12, 11) to 25: planned for the lowest, quests done by some stay for the others"
group trio 25 0 warrior14 priest12 mage11
summary trio
python3 -I - "$out/guides/trio.json" <<'PY'
import json, sys
r = json.load(open(sys.argv[1]))
steps = r["steps"]
print("players:", ", ".join(p["name"] for p in r["players"]))
print("steps for some players only: %d of %d" % (sum(1 for s in steps if s.get("members")), len(steps)))
PY
"$fg" route --race human --class mage --start "$out/mage11.json" --to-level 25 --effort 0 --routes "$out/solo" \
    --addon-dir "$out/addon" --name solo-mage25 >"$out/solo-mage25.log" 2>&1
echo "the lowest alone: $(grep -m1 '^solo-mage25:' "$out/solo-mage25.log" | sed 's/.*level/level/')"

echo "== wide gap (12 and 24) to 30: do they ever play together?"
group gap 30 0 priest12 rogue24
summary gap

if [ "${1:-}" = sync ]; then
    echo "== levels kept together by the planner (effort 1)"
    group sync-on 25 1 warrior14 priest12 mage11
    group sync-off 25 1 warrior14 priest12 mage11 -- --param group_sync_weight=0 --param group_sync_max=100
    summary sync-on
    summary sync-off
fi
