#!/bin/sh
# Publish the addons of pushed main on CurseForge and Wago outside a release, to try them
# (.github/workflows/addons.yml run by hand): beta by default, with their changelog's Unreleased
# section. Releases publish them on their own (scripts/release.sh).
#
#   scripts/publish-addons.sh 0.14.0                        # both, beta
#   scripts/publish-addons.sh 0.14.0 Factoruide_Guide alpha
#   scripts/publish-addons.sh 0.14.0 both release
set -eu
cd "$(dirname "$0")/.."

version=${1:-}
addons=${2:-both}
channel=${3:-beta}
case $version in
    [0-9]*.[0-9]*.[0-9]*) ;;
    *)
        echo "usage: $0 X.Y.Z [both|Factoruide_Profile|Factoruide_Guide] [beta|alpha|release]" >&2
        exit 2
        ;;
esac
case $addons in both | Factoruide_Profile | Factoruide_Guide) ;; *)
    echo "unknown addon: $addons" >&2
    exit 2
    ;;
esac
case $channel in release | beta | alpha) ;; *)
    echo "unknown channel: $channel" >&2
    exit 2
    ;;
esac

git fetch -q origin main
[ -z "$(git status --porcelain addon)" ] || echo "note: uncommitted addon changes are not published" >&2
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || echo "note: publishing origin/main, not HEAD" >&2

for addon in Factoruide_Profile Factoruide_Guide; do
    [ "$addons" = both ] || [ "$addons" = "$addon" ] || continue
    echo "== $addon $version ($channel)"
    scripts/addon-changes.sh "$addon" || echo "(no Unreleased changes)"
done

gh workflow run addons.yml -f ref=main -f version="$version" -f addons="$addons" -f channel="$channel"
echo "started: https://github.com/brouznouf/factoruide/actions/workflows/addons.yml"
