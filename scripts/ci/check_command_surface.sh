#!/usr/bin/env bash
# Copyright (c) 2026, The Shekyl Foundation
#
# All rights reserved.
# BSD-3-Clause
#
# check_command_surface.sh — the GUI's command surface, held to the wallet
# contract in three legs. Sibling of shekyl-core's
# scripts/ci/check_wallet_rpc_liveness.sh, at the Tauri edge instead of the
# RPC edge.
#
# The surface is `tauri::generate_handler![...]` in src-tauri/src/lib.rs: the
# commands the frontend can invoke. Three things go wrong with such a list,
# and each has happened here:
#
#   1. Name leg.     A command is minted under a GUI-only name for an
#                    operation the wallet contract already names, and the
#                    vocabulary drifts (get_address / get_primary_address).
#                    Every registered command is a contract adapter (its name
#                    is SPECIFIED in shekyl-core's x-shekyl-method-registry)
#                    or has a row in command_surface.conf saying what it is
#                    instead: SHELL, RENAME <method>, or COMPOSITE <methods>.
#                    A name the contract REJECTED is never registered.
#   2. Consumer leg. A command is registered that the frontend never invokes
#                    (dead surface shipped by default), or the frontend
#                    invokes a name that is not registered (a button that can
#                    only fail). Both directions are checked. Tests are not
#                    consumers.
#   3. Stub leg.     A registered command's tail is an unconditional refusal:
#                    `Err(...)` or `return Err(...)`. An earlier `return Err`
#                    guarding a real tail is not one (a closed wallet). An
#                    absent feature is absent from the UI, not a registered
#                    refusal. No constant is special-cased.
#
# Feature-gated registrations (`#[cfg(feature = "...")]` in the handler
# list) are not in the default build; legs 2 and 3 do not apply to them and
# they are reported, not judged. The REJECTED check applies to them too.
#
# Rule 47: every extraction asserts its subject exists. An empty handler
# list, an empty registry, a frontend with no invoke, or a handler entry the
# extractor cannot read is a failure, not a pass.
#
# Deliberately grep-cheap — no toolchain, no build.
#
# Environment (for the self-test, scripts/ci/test_check_command_surface.sh):
#   COMMAND_SURFACE_ROOT  the wallet tree to judge (default: this repo)
#   SHEKYL_CORE_ROOT      the shekyl-core checkout holding the contract
#                         (default: ../shekyl-core beside this repo, which is
#                         where ci.yml clones it)

set -euo pipefail

ROOT="${COMMAND_SURFACE_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
CORE="${SHEKYL_CORE_ROOT:-$ROOT/../shekyl-core}"
CONF="$ROOT/scripts/ci/command_surface.conf"
LIB="$ROOT/src-tauri/src/lib.rs"
RUST_SRC="$ROOT/src-tauri/src"
FRONTEND="$ROOT/src"
REGISTRY="$CORE/docs/api/wallet_rpc.yaml"

fail=0
flag() { printf '  %-7s %-30s %s\n' "$1" "$2" "$3"; fail=$((fail + 1)); }

for f in "$LIB" "$CONF"; do
  [[ -f $f ]] || { echo "FAIL: $f not found — has the surface moved?"; exit 2; }
done
[[ -d $RUST_SRC && -d $FRONTEND ]] || { echo "FAIL: src-tauri/src or src missing under $ROOT"; exit 2; }
if [[ ! -f $REGISTRY ]]; then
  echo "FAIL: wallet contract not found at $REGISTRY."
  echo "      This gate reads shekyl-core's x-shekyl-method-registry; ci.yml"
  echo "      clones core beside this repo, and locally the worktree sits beside"
  echo "      it. Set SHEKYL_CORE_ROOT if yours is elsewhere. Not judging."
  exit 2
fi

# ── The contract's registry (status of record, rule 23) ────────────────────
declare -A STATUS=()
while IFS= read -r line; do
  if [[ $line =~ ^[[:space:]]{2}([a-z0-9_]+):[[:space:]]*\{[[:space:]]*status:[[:space:]]*([A-Z]+) ]]; then
    STATUS["${BASH_REMATCH[1]}"]="${BASH_REMATCH[2]}"
  fi
done < <(sed -n '/^x-shekyl-method-registry:/,/^[^ ]/p' "$REGISTRY")
if [[ ${#STATUS[@]} -eq 0 ]]; then
  echo "FAIL: extracted no methods from $REGISTRY's x-shekyl-method-registry."
  echo "      An empty extraction is a failure, not a skip."
  exit 2
fi

# ── The registered surface: generate_handler![...] in lib.rs ───────────────
open_count=$(grep -c 'generate_handler!\[' "$LIB" || true)
if [[ $open_count -ne 1 ]]; then
  echo "FAIL: expected exactly one 'generate_handler![' in $LIB, found $open_count."
  exit 2
fi
declare -A REGISTERED=()   # name -> ungated | gated
declare -a ORDER=()
gated=0
while IFS= read -r line; do
  stripped="${line#"${line%%[![:space:]]*}"}"
  case $stripped in
    ""|//*) continue ;;
    "#[cfg("*) gated=1; continue ;;
  esac
  if [[ $stripped =~ ^([a-z0-9_]+)::([a-z0-9_]+),?$ ]]; then
    name="${BASH_REMATCH[2]}"
    if [[ -n ${REGISTERED[$name]:-} ]]; then
      echo "FAIL: $name registered twice in $LIB."; exit 2
    fi
    if [[ $gated -eq 1 ]]; then REGISTERED[$name]=gated; else REGISTERED[$name]=ungated; fi
    ORDER+=("$name"); gated=0
  else
    echo "FAIL: handler entry this gate cannot read in $LIB: '$stripped'."
    echo "      Entries are 'module::command,' optionally preceded by '#[cfg(...)]'."
    echo "      Extend the extractor deliberately rather than let it skip."
    exit 2
  fi
done < <(awk '/generate_handler!\[/{p=1;next} p&&/^[[:space:]]*\]\)/{exit} p' "$LIB")
if [[ ${#ORDER[@]} -eq 0 ]]; then
  echo "FAIL: extracted no commands from $LIB's generate_handler![...]."; exit 2
fi

# ── What the frontend invokes ──────────────────────────────────────────────
# `invoke("name"` / `invoke<T>("name"`, across lines, including a nested
# generic (`invoke<Record<string, T>>("name")`). Tests are not consumers:
# neither the test directories nor a `*.test.*` / `*.spec.*` file beside its
# subject (src/context/ShardPickerContext.test.tsx is one). The first capture
# is the generic, so (?1) walks nested angle brackets. An invoke whose first
# argument is not a string literal cannot be read here and fails closed.
# Group 1 must stay the generic: (?1) is that group.
readonly INVOKE_LITERAL='\binvoke(<(?:[^<>]++|(?1))*>)?\(\s*"[a-z0-9_]+"'
readonly INVOKE_UNREADABLE='\binvoke(<(?:[^<>]++|(?1))*>)?\(\s*[^"\s)]'
invoke_scan() {
  grep -rPzo --include='*.ts' --include='*.tsx' \
    --exclude-dir=__tests__ --exclude-dir=test \
    --exclude='*.test.ts' --exclude='*.test.tsx' --exclude='*.spec.ts' --exclude='*.spec.tsx' \
    "$1" "$FRONTEND" 2>/dev/null | tr '\0' '\n' || true
}
unreadable=$(invoke_scan "$INVOKE_UNREADABLE" | grep -c . || true)
if [[ $unreadable -ne 0 ]]; then
  echo "FAIL: $unreadable invoke() call(s) whose command is not a string literal:"
  invoke_scan "$INVOKE_UNREADABLE" | sed 's/^/      /'
  echo "      The consumer leg reads literal names; a computed name is invisible to it."
  exit 2
fi
declare -A INVOKED=()
while IFS= read -r name; do
  [[ -n $name ]] && INVOKED[$name]=1
done < <(invoke_scan "$INVOKE_LITERAL" | grep -oP '"[a-z0-9_]+"' | tr -d '"' | sort -u)
if [[ ${#INVOKED[@]} -eq 0 ]]; then
  echo "FAIL: no invoke(\"...\") found under $FRONTEND — the consumer leg has no subject."; exit 2
fi

# ── The dispositions ───────────────────────────────────────────────────────
declare -A DISPOSITION=() DETAIL=()
while read -r name kind detail; do
  [[ -z $name || $name == \#* ]] && continue
  if [[ -n ${DISPOSITION[$name]:-} ]]; then
    echo "FAIL: $name has two rows in $CONF."; exit 2
  fi
  case $kind in
    SHELL)
      [[ -n $detail ]] || { echo "FAIL: SHELL row for $name gives no reason."; exit 2 ;} ;;
    RENAME)
      [[ $detail =~ ^[a-z0-9_]+$ ]] || { echo "FAIL: RENAME row for $name must name exactly one contract method."; exit 2 ;}
      [[ ${STATUS[$detail]:-} == SPECIFIED ]] || { echo "FAIL: RENAME target '$detail' for $name is not SPECIFIED in the contract."; exit 2 ;} ;;
    COMPOSITE)
      [[ -n $detail ]] || { echo "FAIL: COMPOSITE row for $name lists no contract methods."; exit 2 ;}
      for m in $detail; do
        [[ ${STATUS[$m]:-} == SPECIFIED ]] || { echo "FAIL: COMPOSITE input '$m' for $name is not SPECIFIED in the contract."; exit 2 ;}
      done ;;
    *) echo "FAIL: unknown disposition '$kind' for $name in $CONF (SHELL|RENAME|COMPOSITE)."; exit 2 ;;
  esac
  DISPOSITION[$name]=$kind; DETAIL[$name]=$detail
done < "$CONF"

# ── Locate each registered command's body for the stub leg ─────────────────
# The body is the text from `fn <name>(` to the first line that is a bare
# `}`; commands are top-level fns, so that is the fn's own close.
body_of() {
  local name="$1" files
  files=$(grep -lE "^pub (async )?fn ${name}\(" "$RUST_SRC"/*.rs || true)
  [[ $(wc -w <<<"$files") -eq 1 ]] || return 1
  awk -v n="$name" '
    $0 ~ "^pub (async )?fn " n "\\(" { inb = 1 }
    inb { print }
    inb && /^}/ { exit }
  ' "$files"
}

ungated=0 gated_n=0 contract=0 shell=0 rename=0 composite=0
for name in "${ORDER[@]}"; do
  if [[ ${REGISTERED[$name]} == gated ]]; then gated_n=$((gated_n + 1)); else ungated=$((ungated + 1)); fi
done
echo "command surface: ${#ORDER[@]} registered ($ungated default build, $gated_n feature-gated), ${#INVOKED[@]} invoked names, ${#STATUS[@]} contract methods"

# ── Leg 1: name ────────────────────────────────────────────────────────────
for name in "${ORDER[@]}"; do
  status="${STATUS[$name]:-}"
  if [[ $status == REJECTED || $status == RESERVED ]]; then
    flag REFUSED "$name" "the contract marks this $status; it must not be registered (rule 23)"
    continue
  fi
  [[ ${REGISTERED[$name]} == ungated ]] || continue
  if [[ $status == SPECIFIED ]]; then
    contract=$((contract + 1))
    [[ -z ${DISPOSITION[$name]:-} ]] || flag ROW "$name" "is a contract method; its ${DISPOSITION[$name]} row is redundant"
  elif [[ -n ${DISPOSITION[$name]:-} ]]; then
    case ${DISPOSITION[$name]} in
      SHELL) shell=$((shell + 1)) ;;
      RENAME) rename=$((rename + 1)); printf '  %-7s %-30s → %s\n' rename "$name" "${DETAIL[$name]}" ;;
      COMPOSITE) composite=$((composite + 1)) ;;
    esac
  else
    flag UNNAMED "$name" "not a contract method and no row in command_surface.conf"
  fi
done
for name in "${!DISPOSITION[@]}"; do
  [[ ${REGISTERED[$name]:-} == ungated ]] || flag STALE "$name" "row in command_surface.conf but not registered in the default build"
done

# ── Leg 2: consumer ────────────────────────────────────────────────────────
for name in "${ORDER[@]}"; do
  if [[ -z ${INVOKED[$name]:-} ]]; then
    if [[ ${REGISTERED[$name]} == ungated ]]; then
      flag DEAD "$name" "registered but nothing in the frontend invokes it"
    else
      printf '  %-7s %-30s feature-gated and nothing in the frontend invokes it (not judged)\n' gated "$name"
    fi
  fi
done
for name in "${!INVOKED[@]}"; do
  [[ -n ${REGISTERED[$name]:-} ]] || flag UNREG "$name" "invoked from the frontend but not registered"
done

# ── Leg 3: stub ────────────────────────────────────────────────────────────
for name in "${ORDER[@]}"; do
  [[ ${REGISTERED[$name]} == ungated ]] || continue
  if ! body=$(body_of "$name"); then
    flag NOBODY "$name" "cannot locate exactly one 'pub [async] fn $name(' under src-tauri/src"
    continue
  fi
  # The tail, not a named constant. `^}` is the fn's own close; an
  # indented `}` stays, so a match whose last arm is `Err` is not a tail.
  last=$(printf '%s\n' "$body" | grep -v '^[[:space:]]*$' | grep -v '^}' | tail -n 1)
  if [[ $last =~ ^[[:space:]]*(return[[:space:]]+)?Err\( ]]; then
    flag STUB "$name" "tail is an unconditional refusal; an absent feature is absent from the UI"
  fi
done

if [[ $fail -gt 0 ]]; then
  cat <<EOF

FAIL: $fail command-surface violation(s).

Every registered command is a contract adapter or has a row in
scripts/ci/command_surface.conf (SHELL | RENAME <method> | COMPOSITE
<methods>); every registered command is invoked from the frontend and
every invoked name is registered; no registered command's tail is a
refusal. Fix the surface — wire the caller, delete the command, or name
what it is — rather than the gate.
Policy: .cursor/rules/28-command-surface.mdc
EOF
  exit 1
fi

echo "command surface: $contract contract adapters, $shell shell, $rename rename (drift ledger), $composite composite — three legs hold"
