#!/bin/sh
# Build fg for profiling (target/profiling/fg) and optionally record a run with perf.
#
#   scripts/profile.sh build
#   scripts/profile.sh record OUT.svg -- FG_ARGS...
#
# Frame pointers make `perf record --call-graph fp` cheap enough with 8 search threads (dwarf
# unwinding is far too heavy). RUSTFLAGS must stay the same on every call, or Cargo rebuilds the
# whole target/profiling directory. Routes and addon files go to /tmp, never to the user's.
set -eu
cd "$(dirname "$0")/.."
export RUSTFLAGS="-C force-frame-pointers=yes"

build() {
    cargo build --profile profiling -p fg
}

case ${1:-} in
build)
    build
    ;;
record)
    [ $# -ge 3 ] && [ "$3" = "--" ] || { sed -n '4,5p' "$0" | cut -c3- >&2; exit 2; }
    out=$2
    shift 3
    build
    # perf data next to the flamegraph, for `perf report -i`.
    data=${out%.svg}.perf
    export FG_ROUTES=${FG_ROUTES:-/tmp/fg-profile/routes} FG_ADDON_DIR=${FG_ADDON_DIR:-/tmp/fg-profile/addon}
    mkdir -p "$FG_ROUTES" "$FG_ADDON_DIR"
    perf record -F 499 --call-graph fp -o "$data" target/profiling/fg "$@"
    flamegraph --perfdata "$data" -o "$out"
    echo "Flamegraph: $out, perf data: $data"
    ;;
*)
    sed -n '4,5p' "$0" | cut -c3- >&2
    exit 2
    ;;
esac
