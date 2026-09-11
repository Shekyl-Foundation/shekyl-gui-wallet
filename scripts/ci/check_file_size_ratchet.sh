#!/usr/bin/env bash
# Copyright (c) 2026, The Shekyl Foundation
#
# All rights reserved.
# BSD-3-Clause
#
# check_file_size_ratchet.sh — structural anti-drift guard for the GUI wallet.
#
# Greenfield composition discipline: modules stay small, projections live
# outside the session shell, pages compose panels. Review does not catch
# accretion; this tripwire makes the 1k-line rule mechanical.
#
# Template: shekyl-core scripts/ci/check_engine_decomposition.sh
# (bidirectional FILE ceilings + NEW_FILE_CAP + BAND slack).
#
# Policy: .cursor/rules/27-composition-decomposition.mdc
# Baselines: scripts/ci/file_size_ratchet.conf
#
# Exit 0 = clean. Non-zero = at least one of:
#   - a baselined file regressed above its ceiling
#   - a baselined file dropped >BAND below its ceiling (tighten it)
#   - a non-baselined production file crossed NEW_FILE_CAP (new god-file)
#   - a FILE baseline names a path that no longer exists (stale entry)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CONF="${REPO_ROOT}/scripts/ci/file_size_ratchet.conf"

[ -f "${CONF}" ] || { echo "FATAL: ratchet conf missing at ${CONF}"; exit 2; }

fail=0
note() { echo "  - $1"; }

# --- parse the conf ----------------------------------------------------------
NEW_FILE_CAP=0
BAND=0
declare -A FILE_CEIL
declare -A EXCLUDE
while read -r key a b _rest; do
  case "${key}" in
    ''|\#*)          continue ;;
    NEW_FILE_CAP)    NEW_FILE_CAP="${a}" ;;
    BAND)            BAND="${a}" ;;
    FILE)            FILE_CEIL["${a}"]="${b}" ;;
    EXCLUDE)         EXCLUDE["${a}"]=1 ;;
    *)               echo "FATAL: unknown conf key '${key}'"; exit 2 ;;
  esac
done < "${CONF}"

if [ "${NEW_FILE_CAP}" -le 0 ] || [ "${BAND}" -le 0 ]; then
  echo "FATAL: NEW_FILE_CAP and BAND must be positive integers in ${CONF}"
  exit 2
fi

# True if repo-relative path is under an EXCLUDE prefix or exact EXCLUDE key.
is_excluded() {
  local rel="$1"
  local key
  for key in "${!EXCLUDE[@]}"; do
    if [ "${rel}" = "${key}" ] || [[ "${rel}" == "${key}/"* ]]; then
      return 0
    fi
  done
  # Convention: *.test.ts / *.test.tsx are test harness even outside __tests__/
  case "${rel}" in
    *.test.ts|*.test.tsx|*.test.js|*.test.jsx) return 0 ;;
  esac
  return 1
}

# Collect production sources under the two roots.
# Pure find — no ripgrep dependency (matches shekyl-core CI gates).
mapfile -t FILES < <(
  {
    find "${REPO_ROOT}/src-tauri/src" -type f -name '*.rs' 2>/dev/null
    find "${REPO_ROOT}/src" -type f \( -name '*.ts' -o -name '*.tsx' \) 2>/dev/null
  } | sort
)

if [ "${#FILES[@]}" -eq 0 ]; then
  echo "FATAL: no source files found under src-tauri/src or src"
  exit 2
fi

# --- per-file line-count ratchet + new-file cap ------------------------------
for path in "${FILES[@]}"; do
  rel="${path#"${REPO_ROOT}/"}"
  if is_excluded "${rel}"; then
    continue
  fi
  # Skip entrypoints and generated noise that are not composition roots.
  case "${rel}" in
    src-tauri/src/main.rs|src/main.tsx|src/vite-env.d.ts) continue ;;
  esac

  lines="$(wc -l < "${path}" | tr -d ' ')"

  if [ -n "${FILE_CEIL[${rel}]:-}" ]; then
    ceil="${FILE_CEIL[${rel}]}"
    if [ "${lines}" -gt "${ceil}" ]; then
      echo "FAIL: ${rel} is ${lines} lines (ceiling ${ceil}) — regression."
      note "Split by workflow / panel / projection module instead of growing the god-file."
      note "See .cursor/rules/27-composition-decomposition.mdc"
      fail=1
    elif [ "${lines}" -lt "$(( ceil - BAND ))" ]; then
      echo "FAIL: ${rel} is ${lines} lines, >${BAND} under ceiling ${ceil} — tighten it."
      note "Lower its FILE line in scripts/ci/file_size_ratchet.conf to lock the win in."
      fail=1
    fi
  elif [ "${lines}" -gt "${NEW_FILE_CAP}" ]; then
    echo "FAIL: ${rel} is ${lines} lines (>${NEW_FILE_CAP}) and is not baselined — new god-file."
    note "Carve it first, or add a reviewed FILE baseline in scripts/ci/file_size_ratchet.conf."
    note "See .cursor/rules/27-composition-decomposition.mdc"
    fail=1
  fi
done

# --- stale baseline entries --------------------------------------------------
for key in "${!FILE_CEIL[@]}"; do
  if [ ! -f "${REPO_ROOT}/${key}" ]; then
    echo "FAIL: ratchet baselines ${key} but ${key} no longer exists — stale entry."
    note "Remove its FILE line from scripts/ci/file_size_ratchet.conf."
    fail=1
  fi
done

if [ "${fail}" -ne 0 ]; then
  echo "check_file_size_ratchet: FAILED"
  exit 1
fi
echo "check_file_size_ratchet: clean (NEW_FILE_CAP=${NEW_FILE_CAP}, BAND=${BAND})"
