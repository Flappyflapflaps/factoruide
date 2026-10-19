#!/bin/sh
# Publish a new version: tag the pushed main commit vX.Y.Z with the release notes and push the
# tag. The CI (.github/workflows/release.yml) builds the Windows installer, the macOS apps and the
# Linux app and publishes them (installed apps offer the update at startup, showing the notes),
# deploys the web app, and publishes each addon whose version is the tag's on CurseForge and Wago.
# An addon changed since the last release gets the new version first (a commit pushed on main):
# the apps built from the tag then ask for it; an unchanged one keeps its version and is not
# published again.
#
#   scripts/release.sh 0.2.0 "Short notes (shown in the app): a link to CHANGELOG.md"
#   scripts/release.sh 0.2.0            # writes the notes in $EDITOR
set -eu
cd "$(dirname "$0")/.."

version=${1:-}
case $version in
    [0-9]*.[0-9]*.[0-9]*) ;;
    *)
        echo "usage: $0 X.Y.Z [notes]" >&2
        exit 2
        ;;
esac
tag="v$version"

[ "$(git branch --show-current)" = main ] || { echo "not on main" >&2; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "uncommitted changes" >&2; exit 1; }
git fetch -q origin main --tags
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || { echo "main differs from origin/main: push or pull first" >&2; exit 1; }
if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
    echo "$tag already exists" >&2
    exit 1
fi

# The guide this release's apps make, kept for the addon's tests (a newer addon must read it).
# Not released yet: made again from what is committed now.
rm -f "addon/tests/guides/$version.lua"
scripts/guide-fixture.sh "$version"
if [ -n "$(git status --porcelain addon/tests/guides)" ]; then
    git add addon/tests/guides
    git commit -q -m "Addon tests: the guide of $version" -- addon/tests/guides
    git push -q origin main
fi

# The addons changed since the last release take this version.
last=$(git describe --tags --abbrev=0 --match 'v*.*.*' 2>/dev/null || true)
bumped=
for addon in Factoruide_Profile Factoruide_Guide; do
    toc="addon/$addon/$addon.toc"
    if [ -z "$last" ] || ! git diff --quiet "$last" HEAD -- "addon/$addon"; then
        if ! grep -q "^## Version: $version\$" "$toc"; then
            sed -i "s/^## Version: .*/## Version: $version/" "$toc"
            bumped="$bumped $addon"
        fi
    fi
done
if [ -n "$bumped" ]; then
    git commit -q -m "Addons $version:$bumped" -- addon/*/*.toc
    git push -q origin main
    echo "addons $version:$bumped"
fi

if [ $# -ge 2 ]; then
    git tag -a "$tag" -m "$2"
else
    git tag -a "$tag"
fi
git push origin "$tag"
echo "$tag pushed: https://github.com/brouznouf/factoruide/actions/workflows/release.yml"
