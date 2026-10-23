#!/bin/sh
# The addon's smoke tests (luajit): the installed luajit, or a Docker image built once.
#
#   scripts/addon-test.sh            both suites (guide, then profile alone)
#   scripts/addon-test.sh profile    one of them
set -eu
cd "$(dirname "$0")/.."

run() {
    if command -v luajit >/dev/null 2>&1; then
        luajit addon/tests/smoke.lua "$@"
    else
        if ! docker image inspect factoruide-luajit >/dev/null 2>&1; then
            printf 'FROM ubuntu:22.04\nRUN apt-get update -qq && apt-get install -y -qq luajit && rm -rf /var/lib/apt/lists/*\n' |
                docker build -q -t factoruide-luajit - >/dev/null
        fi
        docker run --rm -v "$PWD":/w -w /w factoruide-luajit luajit addon/tests/smoke.lua "$@"
    fi
}

if [ $# -gt 0 ]; then
    run "$@" | tail -3
else
    run | tail -3
    run profile | tail -3
fi
