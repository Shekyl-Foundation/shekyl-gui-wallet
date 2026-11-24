#!/usr/bin/env bash
# Copyright (c) 2026, The Shekyl Foundation
#
# All rights reserved.
# BSD-3-Clause
#
# Which shekyl-core this wallet is built against.
#
#   core_ref.sh <branch> <target>   the core ref for a CI run
#   core_ref.sh --pin               the core tag this wallet's release pairs with
#
# <branch> is the branch under test: a pull request's head, or the branch
# pushed. <target> is where that code is going: a pull request's base, or
# the branch pushed.
#
#   1. A work branch with a shekyl-core branch of the same exact name builds
#      against it. That is how a paired change and a release branch are tested.
#   2. Code going to `main` builds against the release pin, the pair that ships.
#   3. Everything else builds against core `dev`.
#
# `main` and `dev` never pair by name: a main-to-dev back-merge has `main` as
# its head and lands on `dev`, so its target decides.
#
# The pin is one line in scripts/release/core-release-ref. Bump it when
# pairing the next wallet tag with a new shekyl-core tag.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PIN_FILE="$ROOT/scripts/release/core-release-ref"
CORE_URL="${SHEKYL_CORE_URL:-https://github.com/Shekyl-Foundation/shekyl-core.git}"

release_pin() {
  local pin
  pin="$(<"$PIN_FILE")"
  if [[ ! "$pin" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]]; then
    echo "core_ref: $PIN_FILE does not hold one release tag" >&2
    return 1
  fi
  printf '%s\n' "$pin"
}

if [ "$#" -eq 1 ] && [ "$1" = "--pin" ]; then
  release_pin
  exit 0
fi

if [ "$#" -ne 2 ] || [ -z "$1" ] || [ -z "$2" ]; then
  echo "usage: core_ref.sh <branch> <target> | core_ref.sh --pin" >&2
  exit 2
fi

branch="$1"
target="$2"

case "$branch" in
  main | dev) ;;
  *)
    # The exact branch: a bare name also matches `*/<name>`. A failed lookup
    # stops the run, so a network fault can't quietly test against `dev`.
    hits="$(git ls-remote --heads "$CORE_URL" "refs/heads/$branch")"
    if [ -n "$hits" ]; then
      printf '%s\n' "$branch"
      exit 0
    fi
    ;;
esac

case "$target" in
  main) release_pin ;;
  *) echo dev ;;
esac
