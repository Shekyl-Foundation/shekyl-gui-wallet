#!/usr/bin/env python3
# Copyright (c) 2026, The Shekyl Foundation
#
# All rights reserved.
# BSD-3-Clause
#
# The wallet's lock holds at most one shekyl-spawn (Rick, 2026-11-13;
# shekyl-core #1372).
#
# shekyl-spawn is the process's one spawn lock. Its contract is one lock per
# process, and a holder only spawns under it. A binary that linked two copies
# would hold two locks: a spawn under one would not exclude a spawn under the
# other, and on Windows a child could inherit the end of a pair another launch
# was handing over (shekyl-core TOR_SUPERVISOR.md §4.7, TSR-F16). The wallet
# links shekyl-spawn through core's Tor client, and through its own launch of
# the daemon (GUI_SYNC_LANE.md GS-C3), so two versions in the lock would be two
# copies in the binary.
#
# The subject is src-tauri/Cargo.lock. It must exist and list packages
# (rule 47), or the check has nothing to read and fails. Zero copies passes:
# a wallet built against a core that predates the crate links none.
#
# Usage:  check_one_spawn_lock.py [LOCK]     # default: src-tauri/Cargo.lock
#         check_one_spawn_lock.py --selftest # each failure bites

import pathlib
import re
import sys
import tempfile

CRATE = "shekyl-spawn"
PACKAGE = re.compile(r"^\[\[package\]\]\s*$", re.M)
NAME = re.compile(r'^name = "([^"]+)"\s*$', re.M)


def copies(text):
    """(packages in the lock, versions of the crate)."""
    blocks = PACKAGE.split(text)[1:]
    found = []
    for block in blocks:
        name = NAME.search(block)
        if name and name.group(1) == CRATE:
            version = re.search(r'^version = "([^"]+)"\s*$', block, re.M)
            found.append(version.group(1) if version else "?")
    return len(blocks), found


def check(lock):
    try:
        text = pathlib.Path(lock).read_text(encoding="utf-8")
    except OSError as e:
        return [f"cannot read {lock}: {e}"]
    packages, found = copies(text)
    if packages == 0:
        return [f"{lock} lists no packages; the check has no subject"]
    if len(found) > 1:
        return [
            f"{lock} holds {len(found)} copies of {CRATE} ({', '.join(found)}); "
            "one process must hold one spawn lock"
        ]
    return []


def selftest():
    one = '[[package]]\nname = "shekyl-spawn"\nversion = "3.1.0"\n'
    other = '[[package]]\nname = "serde"\nversion = "1.0.0"\n'
    cases = [
        ("no copy passes", other, 0),
        ("one copy passes", other + one, 0),
        ("two copies fail", other + one + one.replace("3.1.0", "3.2.0"), 1),
        ("an empty lock fails", "", 1),
        ("a name in a dependency list is not a package", other + 'dependencies = [\n "shekyl-spawn",\n]\n' + one, 0),
    ]
    failures = 0
    with tempfile.TemporaryDirectory() as tmp:
        for title, text, expect in cases:
            path = pathlib.Path(tmp) / "Cargo.lock"
            path.write_text(text, encoding="utf-8")
            got = len(check(path))
            ok = got == expect
            print(f"{'ok' if ok else 'FAIL'}: {title}" + ("" if ok else f" (got {got} errors)"))
            failures += 0 if ok else 1
    return 1 if failures else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    lock = argv[0] if argv else pathlib.Path(__file__).resolve().parents[2] / "src-tauri" / "Cargo.lock"
    errors = check(lock)
    for e in errors:
        print(f"error: {e}", file=sys.stderr)
    if errors:
        return 1
    _, found = copies(pathlib.Path(lock).read_text(encoding="utf-8"))
    noun = "copy" if len(found) == 1 else "copies"
    print(f"spawn lock: {len(found)} {noun} of {CRATE} in {lock}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
