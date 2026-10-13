#!/bin/sh
# Paired measures of the planner: the same starts and seeds under two settings, compared.
#
#   scripts/measure.sh run NAME [--effort E] [--jobs N] [--to-level L] [--param k=v]...
#   scripts/measure.sh compare BASE OTHER
#   scripts/measure.sh race NAME
#
# `run` plans every (start, seed) once (the tournament is deterministic) into
# $FG_MEASURE/NAME (default /tmp/fg-measure), N at a time (8 threads each), skipping those done.
# `compare` prints, for each pair, the play time of BASE, of OTHER and the difference, then the
# mean difference and its standard error (a gain is real when it is well over twice the error).
# `race` tells how soon the race ranking settles (see below).
# FG_STARTS ("race:class ...") and FG_SEEDS ("1000 2000 ...") change the six classic starts and
# the four seeds; seeds are far apart, candidate k of seed s has seed s + k.
set -eu
cd "$(dirname "$0")/.."

root=${FG_MEASURE:-/tmp/fg-measure}
starts=${FG_STARTS:-"orc:warlock human:mage dwarf:hunter nightelf:druid undead:priest tauren:warrior"}
seeds=${FG_SEEDS:-"1000 2000 3000 4000"}

usage() {
    sed -n '4,6p' "$0" | cut -c3- >&2
    exit 2
}

run() {
    [ $# -ge 1 ] || usage
    name=$1
    shift
    effort=1
    jobs=3
    to=60
    params=""
    while [ $# -gt 0 ]; do
        case $1 in
        --effort) effort=$2 ;;
        --jobs) jobs=$2 ;;
        --to-level) to=$2 ;;
        --param) params="$params --param $2" ;;
        *) usage ;;
        esac
        shift 2
    done
    dir=$root/$name
    mkdir -p "$dir"
    echo "effort=$effort to=$to$params" >"$dir/settings"
    cargo build --release --quiet -p fg
    # Its own binary: a rebuild meanwhile does not change the runs left.
    cp target/release/fg "$dir/fg"
    for start in $starts; do
        for seed in $seeds; do
            echo "$start $seed"
        done
    done | xargs -P "$jobs" -L 1 sh -c '
        dir=$1 effort=$2 to=$3 params=$4 start=$5 seed=$6
        id=$(echo "$start" | tr : -)-$seed
        [ -f "$dir/$id.json" ] && exit 0
        work=$dir/.$id
        mkdir -p "$work"
        began=$(date +%s)
        # shellcheck disable=SC2086
        if FG_RACE_LOG=1 "$dir/fg" --edition classic --routes "$work" --addon-dir "$work" \
            route --race "${start%%:*}" --class "${start#*:}" --to-level "$to" --effort "$effort" \
            --param "seed=$seed" $params --name r >"$dir/$id.log" 2>&1; then
            echo "elapsed $(($(date +%s) - began)) s" >>"$dir/$id.log"
            mv "$work/r.json" "$dir/$id.json"
            rm -rf "$work"
            echo "$id $(jq -r .total_time_text "$dir/$id.json")"
        else
            echo "$id failed (see $dir/$id.log)" >&2
        fi
    ' sh "$dir" "$effort" "$to" "$params"
}

compare() {
    [ $# -eq 2 ] || usage
    for start in $starts; do
        for seed in $seeds; do
            id=$(echo "$start" | tr : -)-$seed
            a=$root/$1/$id.json
            b=$root/$2/$id.json
            [ -f "$a" ] && [ -f "$b" ] || continue
            echo "$id $(jq .total_time "$a") $(jq .total_time "$b")"
        done
    done | awk -v base="$1" -v other="$2" '
        function hm(s,  m) { m = int((s < 0 ? -s : s) / 60 + 0.5); return sprintf("%s%dh%02d", s < 0 ? "-" : "", int(m / 60), m % 60) }
        BEGIN { printf "%-22s %9s %9s %9s\n", "", base, other, "diff" }
        { d = $3 - $2; n++; sum += d; sq += d * d
          printf "%-22s %9s %9s %9s\n", $1, hm($2), hm($3), hm(d) }
        END {
            if (n == 0) { print "no pairs"; exit 1 }
            mean = sum / n
            sd = n > 1 ? sqrt((sq - n * mean * mean) / (n - 1)) : 0
            printf "%d pairs: mean %s, standard error %s (positive: %s is longer)\n", n, hm(mean), hm(sd / sqrt(n)), other
        }'
    # Calculation time (the runs share the machine: indicative).
    for name in "$1" "$2"; do
        cat "$root/$name"/*.log | awk -v name="$name" '/^elapsed/ { n++; t += $2 } END { if (n) printf "%s: %.0f s per run\n", name, t / n }'
    done
}

# How soon the race ranking settles: at each checkpoint of its tries, the rank of the candidate
# that wins the race, and what keeping the leader of that moment would have lost at the end.
race() {
    [ $# -eq 1 ] || usage
    for log in "$root/$1"/*.log; do
        awk -v id="$(basename "$log" .log)" '
            function minutes(t,  p) { split(t, p, "h"); return p[1] * 60 + p[2] }
            / at [0-9]+ tries: / { k = $2; c = $4; at[c, k] = $NF; cs[c] = 1 }
            /^candidate [0-9]+: [0-9]+h[0-9]+$/ { k = $2; sub(":", "", k); end[k] = minutes($3); if (winner == "") winner = k }
            END {
                for (c in cs) {
                    rank = 1; leader = winner
                    for (key in at) {
                        split(key, p, SUBSEP)
                        if (p[1] != c) continue
                        if (at[c, p[2]] < at[c, winner]) rank++
                        if (at[c, p[2]] < at[c, leader]) leader = p[2]
                    }
                    print id, c, rank, end[leader] - end[winner]
                }
            }' "$log"
    done | sort -k2,2n -k1,1 | awk '
        { n[$2]++; r[$2] += $3; if ($3 > m[$2]) m[$2] = $3; l[$2] += $4; if ($3 <= 8) top8[$2]++; if (!($2 in o)) { o[$2] = ++cn; cs[cn] = $2 } }
        END {
            printf "%8s %6s %16s %16s %14s\n", "tries", "runs", "winner rank avg", "winner rank max", "leader loss avg"
            for (i = 1; i <= cn; i++) {
                c = cs[i]
                printf "%8d %6d %16.1f %16d %11.0f min   (winner in top 8: %d/%d)\n", c, n[c], r[c] / n[c], m[c], l[c] / n[c], top8[c], n[c]
            }
        }'
}

[ $# -ge 1 ] || usage
cmd=$1
shift
case $cmd in
run) run "$@" ;;
compare) compare "$@" ;;
race) race "$@" ;;
*) usage ;;
esac
