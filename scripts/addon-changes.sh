#!/bin/sh
# Print a section of an addon's CHANGELOG.md: a version's, or Unreleased without one. Fails when
# the section is missing or empty. Used for the CurseForge and Wago changelog (addons.yml).
#
#   scripts/addon-changes.sh Factoruide_Guide 0.14.0
#   scripts/addon-changes.sh Factoruide_Profile
set -eu
cd "$(dirname "$0")/.."

case ${1:-} in
    Factoruide_Profile | Factoruide_Guide) ;;
    *)
        echo "usage: $0 Factoruide_Profile|Factoruide_Guide [X.Y.Z]" >&2
        exit 2
        ;;
esac

awk -v h="${2:-Unreleased}" '
    /^## / {
        if (on) exit
        on = ($2 == h)
        next
    }
    on { lines[++n] = $0 }
    END {
        while (n > 0 && lines[n] ~ /^[[:space:]]*$/) n--
        s = 1
        while (s <= n && lines[s] ~ /^[[:space:]]*$/) s++
        for (i = s; i <= n; i++) print lines[i]
        exit s > n
    }
' "addon/$1/CHANGELOG.md"
