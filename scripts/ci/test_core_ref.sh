#!/usr/bin/env bash
# Copyright (c) 2026, The Shekyl Foundation
#
# All rights reserved.
# BSD-3-Clause
#
# Controls for core_ref.sh (rule 50), against a scratch core repository and
# a scratch pin: every row of the selection, and every input it must refuse.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

PIN=v9.9.9-alpha.1
TREE="$WORK/tree"
GATE="$TREE/scripts/ci/core_ref.sh"
PIN_FILE="$TREE/scripts/release/core-release-ref"
mkdir -p "$TREE/scripts/ci" "$TREE/scripts/release"
cp "$ROOT/scripts/ci/core_ref.sh" "$GATE"
printf '%s\n' "$PIN" >"$PIN_FILE"

git init -q -b dev "$WORK/core"
git -C "$WORK/core" -c commit.gpgsign=false -c user.name=control \
  -c user.email=control@example.invalid commit -q --allow-empty -m control
for name in main feat/paired release-v9 other/feat/suffix; do
  git -C "$WORK/core" branch "$name"
done
export SHEKYL_CORE_URL="$WORK/core"

failed=0

selects() {
  local want="$1" branch="$2" target="$3" got
  if got="$("$GATE" "$branch" "$target" 2>&1)" && [ "$got" = "$want" ]; then
    echo "ok: $branch -> $target builds against $want"
  else
    echo "FAIL: $branch -> $target gave '$got', want '$want'" >&2
    failed=1
  fi
}

refuses() {
  local what="$1" got
  shift
  if got="$("$@" 2>/dev/null)"; then
    echo "FAIL: $what was accepted, printing '$got'" >&2
    failed=1
  elif [ -n "$got" ]; then
    echo "FAIL: $what was refused but still printed '$got'" >&2
    failed=1
  else
    echo "ok: $what is refused"
  fi
}

selects dev dev dev
selects "$PIN" main main
selects dev main dev
selects "$PIN" dev main
selects feat/paired feat/paired dev
selects release-v9 release-v9 main
selects dev feat/unpaired dev
selects "$PIN" feat/unpaired main
selects dev feat/suffix dev

marker="$WORK/spliced"
selects dev "\$(touch $marker)" dev
if [ -e "$marker" ]; then
  echo "FAIL: a branch name ran as a command" >&2
  failed=1
fi

if [ "$("$GATE" --pin)" = "$PIN" ]; then
  echo "ok: --pin prints the pin"
else
  echo "FAIL: --pin did not print $PIN" >&2
  failed=1
fi

refuses "no arguments" "$GATE"
refuses "one branch alone" "$GATE" feat/paired
refuses "an empty target" "$GATE" feat/paired ""
SHEKYL_CORE_URL="$WORK/no-such-core" refuses "a core that cannot be read" \
  "$GATE" feat/unpaired dev

for bad in "" "dev" "v3.1" "v3.1.0 v3.1.1" "v3.1.0;true" "--upload-pack=x"; do
  printf '%s\n' "$bad" >"$PIN_FILE"
  refuses "the pin '$bad'" "$GATE" --pin
  refuses "the pin '$bad' for code going to main" "$GATE" main main
done
rm "$PIN_FILE"
refuses "a missing pin" "$GATE" --pin

# The committed pin is one the script accepts.
if ! "$ROOT/scripts/ci/core_ref.sh" --pin >/dev/null; then
  echo "FAIL: the committed pin is not a release tag" >&2
  failed=1
fi

if [ "$failed" -ne 0 ]; then
  echo "core_ref controls FAILED" >&2
  exit 1
fi
echo "core_ref controls passed"
