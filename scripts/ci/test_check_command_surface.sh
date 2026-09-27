#!/usr/bin/env bash
# Copyright (c) 2026, The Shekyl Foundation
#
# All rights reserved.
# BSD-3-Clause
#
# Negative controls for check_command_surface.sh (rule 50): each case is the
# one edit that must turn a leg red, applied to a scratch copy of the tree,
# with the unmutated copy as the control. A gate that cannot fail is not a
# gate; this is where that is proven, on every run.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GATE="$ROOT/scripts/ci/check_command_surface.sh"
CORE="${SHEKYL_CORE_ROOT:-$ROOT/../shekyl-core}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

LIB=src-tauri/src/lib.rs
CMDS=src-tauri/src/commands.rs
CONF=scripts/ci/command_surface.conf
PAGE=src/pages/Settings.tsx

fresh() {
  rm -rf "$WORK/tree"
  mkdir -p "$WORK/tree/scripts/ci" "$WORK/tree/src-tauri"
  cp -r "$ROOT/src" "$WORK/tree/src"
  cp -r "$ROOT/src-tauri/src" "$WORK/tree/src-tauri/src"
  cp "$ROOT/$CONF" "$WORK/tree/$CONF"
}

# Register `name` (module `commands`) with a body, as an ungated entry.
register() {
  local name="$1" body="$2"
  printf '\n#[tauri::command]\npub async fn %s() -> Result<(), String> {\n%s\n}\n' "$name" "$body" >>"$WORK/tree/$CMDS"
  sed -i "s|^\(\s*\)commands::get_wallet_dir,|\1commands::get_wallet_dir,\n\1commands::${name},|" "$WORK/tree/$LIB"
}
invoke_from_page() { printf '\nexport const __probe = () => invoke("%s");\n' "$1" >>"$WORK/tree/$PAGE"; }
allow() { printf '%s SHELL self-test probe\n' "$1" >>"$WORK/tree/$CONF"; }

# Replace the body of `pub async fn <name>` in commands.rs. `body` is the
# new interior, including its leading newline-free indentation.
replace_body() {
  local name="$1" body="$2"
  python3 - "$WORK/tree/$CMDS" "$name" "$body" <<'PY'
import re, sys, pathlib
path, name, body = sys.argv[1:]
file = pathlib.Path(path)
text = file.read_text()
match = re.search(
    rf'^pub async fn {re.escape(name)}\([^{{]*\{{\n(.*?)^\}}',
    text,
    re.S | re.M,
)
assert match, f"{name} body"
file.write_text(text[: match.start(1)] + body + "\n" + text[match.end(1) :])
PY
}

failed=0
expect() {
  local label="$1" want_rc="$2" want_text="$3" rc=0 out
  out="$(COMMAND_SURFACE_ROOT="$WORK/tree" SHEKYL_CORE_ROOT="$CORE" "$GATE" 2>&1)" || rc=$?
  if [[ $rc -ne $want_rc ]]; then
    echo "FAIL: $label (want rc=$want_rc, got $rc)" >&2; printf '%s\n' "$out" >&2; failed=1; return
  fi
  if [[ -n $want_text ]] && ! grep -qF -- "$want_text" <<<"$out"; then
    echo "FAIL: $label (rc ok, but output lacks '$want_text')" >&2; printf '%s\n' "$out" >&2; failed=1; return
  fi
  echo "PASS: $label (rc=$rc)"
}

fresh
expect "control: the unmutated tree holds all three legs" 0 "three legs hold"

# Leg 2 + 3: the edit the gate was built for — re-adding a registered
# refusal nothing calls.
fresh; register scanner_freeze '    Err("not available".into())'; allow scanner_freeze
expect "re-add scanner_freeze: dead and a stub" 1 "STUB    scanner_freeze"
expect "re-add scanner_freeze: dead and a stub (consumer leg too)" 1 "DEAD    scanner_freeze"

# Leg 2, registered → invoked.
fresh; register orphan_probe '    Ok(())'; allow orphan_probe
expect "register a command with no call site" 1 "DEAD    orphan_probe"

# Leg 2, invoked → registered.
fresh; invoke_from_page ghost_probe
expect "a page invokes a name that is not registered" 1 "UNREG   ghost_probe"

# A test file beside its subject is not a consumer: an invoke there neither
# satisfies the consumer leg for a dead command nor trips it for a ghost.
fresh; printf '\nimport { invoke } from "@tauri-apps/api/core";\nexport const __probe = () => invoke("ghost_probe");\n' >"$WORK/tree/src/probe.test.tsx"
expect "an invoke in a *.test.tsx file outside __tests__ is not a consumer" 0 "three legs hold"
fresh; register orphan_probe '    Ok(())'; allow orphan_probe
printf '\nimport { invoke } from "@tauri-apps/api/core";\nexport const __probe = () => invoke("orphan_probe");\n' >"$WORK/tree/src/probe.spec.ts"
expect "a *.spec.ts caller does not rescue a dead command" 1 "DEAD    orphan_probe"

# Leg 3 alone: a live, consumed command whose body becomes a refusal.
# Both shapes this tree writes: a bare `Err(...)` tail, and `return Err(...)`.
fresh
replace_body get_wallet_dir '    Err("stubbed".into())'
expect "keep a stub behind a live command" 1 "STUB    get_wallet_dir"
fresh
replace_body get_wallet_dir '    return Err("stubbed".into());'
expect "a live command whose tail is return Err" 1 "STUB    get_wallet_dir"
# The shape rustfmt gives a long refusal: the tail ends in `))`, not `Err(`.
fresh
replace_body get_wallet_dir '    Err(format!(
        "not available on the Engine backend yet: {}",
        "stubbed"
    ))'
expect "a multi-line Err(format!(...)) tail" 1 "STUB    get_wallet_dir"
fresh
replace_body get_wallet_dir '    return Err(format!(
        "not available: {}",
        "stubbed"
    ));'
expect "a multi-line return Err(...) tail" 1 "STUB    get_wallet_dir"
# A real tail after an early refusal is not a stub.
fresh
replace_body get_wallet_dir '    if false {
        return Err("closed".into());
    }
    Ok(shekyl_wallet::WalletDirResponse {
        dir: String::new(),
        fallback_from: None,
    })'
expect "an early return Err guarding a real multi-line tail" 0 "three legs hold"

# Leg 2: a literal that can never be a command name is a call that can only
# fail — read and flagged, not skipped for failing the name grammar.
fresh; invoke_from_page get-balance
expect "an invoke of a literal that is not a command name" 1 "UNREG   get-balance"

# Leg 1: a name the contract REJECTED, even with a caller and a row.
fresh; register claim '    Ok(())'; invoke_from_page claim; allow claim
expect "register a method the contract REJECTED" 1 "REFUSED claim"

# Leg 1: a registered name with neither a contract match nor a row.
fresh; register orphan_probe '    Ok(())'; invoke_from_page orphan_probe
expect "register a GUI-only name without a disposition" 1 "UNNAMED orphan_probe"

# Leg 1: a row that outlived its command.
fresh; allow vanished_probe
expect "a stale row in command_surface.conf" 1 "STALE   vanished_probe"

# Rows are validated against the contract before any leg runs: a RENAME
# whose target the contract does not specify is refused at parse time.
fresh; printf 'get_balance RENAME no_such_method\n' >>"$WORK/tree/$CONF"
expect "a RENAME target that is not SPECIFIED" 2 "not SPECIFIED"

# Leg 1: a row on a contract method is redundant, and says so.
fresh; printf 'get_balance SHELL redundant row\n' >>"$WORK/tree/$CONF"
expect "a row on a contract-named command" 1 "ROW     get_balance"

# Rule 47: the extractor refuses what it cannot read rather than skipping it.
fresh; sed -i 's|^\(\s*\)commands::get_wallet_dir,|\1commands::get_wallet_dir, commands::get_balance,|' "$WORK/tree/$LIB"
expect "a handler entry the extractor cannot read" 2 "cannot read"

fresh; printf '\nexport const __probe = (n: string) => invoke(n);\n' >>"$WORK/tree/$PAGE"
expect "an invoke with a computed name" 2 "not a string literal"

# A generic that itself contains `>` is still one call. Stopping at the first
# `>` would neither count the literal nor reject the computed name.
fresh
register orphan_probe '    Ok(())'
allow orphan_probe
printf '\nexport const __probe = () => invoke<Record<string, number>>("orphan_probe");\n' >>"$WORK/tree/$PAGE"
expect "a nested generic literal is still a consumer" 0 "three legs hold"
fresh
printf '\nexport const __probe = (n: string) => invoke<Record<string, number>>(n);\n' >>"$WORK/tree/$PAGE"
expect "a computed name inside a nested generic fails closed" 2 "not a string literal"

# Fail closed without the contract.
fresh
rc=0; out="$(COMMAND_SURFACE_ROOT="$WORK/tree" SHEKYL_CORE_ROOT="$WORK/nowhere" "$GATE" 2>&1)" || rc=$?
if [[ $rc -eq 2 ]] && grep -q "wallet contract not found" <<<"$out"; then
  echo "PASS: missing contract fails closed (rc=2)"
else
  echo "FAIL: missing contract (want rc=2 + message, got rc=$rc)" >&2; printf '%s\n' "$out" >&2; failed=1
fi

if [[ $failed -ne 0 ]]; then
  echo "FAIL: check_command_surface.sh negative controls" >&2
  exit 1
fi
echo "check_command_surface.sh: every leg can go red, and the control is green"
