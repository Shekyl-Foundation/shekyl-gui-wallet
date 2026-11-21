#!/usr/bin/env python3
# Copyright (c) 2026, The Shekyl Foundation
#
# All rights reserved.
# BSD-3-Clause
#
# An "unsound" advisory the wallet's audit ignores is checked, so the ignore
# can fail (Rick, 2026-11-21).
#
# src-tauri/.cargo/audit.toml denies "unsound" advisories, and ignores one,
# RUSTSEC-2024-0429 (glib 0.18's VariantStrIter), which Tauri's GTK3 stack
# keeps in the graph. An ignore is a claim that nothing reaches the bug. This
# script holds the claim to the graph: for each row of TABLE (advisory,
# crate, the symbols that reach the bug) it reads `cargo metadata --locked`
# for src-tauri, after `cargo fetch --locked`, and searches the source of
# every resolved package except the named crate itself, plus the wallet's own
# src-tauri/src, for those symbols. A match fails and names the file and
# line.
#
# Rule 47: the scan must have a subject. The named crate must be in the
# graph, and at least one package must have been scanned; otherwise the row
# fails rather than passing as nothing read.
#
# Every ignore in audit.toml is classified from the RustSec advisory database
# cargo-audit has just fetched ($CARGO_HOME/advisory-db, or --advisory-db). An
# ignored id whose advisory is `informational = "unsound"` and has no row
# here is refused: audit.toml cannot carry an unsound ignore that nothing
# checks. An ignored id the database does not hold is refused too, and so is
# a missing database. A row whose advisory is not ignored is stale and
# refused, so the table and audit.toml move together.
#
# Usage:  check_unreached_advisories.py [--advisory-db DIR]
#         check_unreached_advisories.py --selftest   # each failure bites

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # Python before 3.11
    sys.exit("check_unreached_advisories.py needs Python 3.11 or later (tomllib)")

REPO = Path(__file__).resolve().parents[2]
TAURI = REPO / "src-tauri"
AUDIT_TOML = TAURI / ".cargo" / "audit.toml"
OWN_SOURCE = TAURI / "src"

# advisory id -> (crate whose bug it is, symbols that reach the bug).
TABLE: dict[str, tuple[str, tuple[str, ...]]] = {
    # glib 0.15..0.20: VariantStrIter's next/nth/last/next_back/nth_back read
    # a NULL pointer. The iterator is made by Variant::array_iter_str.
    "RUSTSEC-2024-0429": ("glib", ("array_iter_str", "VariantStrIter")),
}

SOURCE_SUFFIXES = (".rs",)


class Refused(Exception):
    """A verdict: the check fails, with the reason."""


def ignored_ids(audit_toml: Path) -> list[str]:
    if not audit_toml.is_file():
        raise Refused(f"{audit_toml} is missing: there is no ignore list to check")
    config = tomllib.loads(audit_toml.read_text(encoding="utf-8"))
    ignore = config.get("advisories", {}).get("ignore", [])
    if not isinstance(ignore, list) or not all(isinstance(i, str) for i in ignore):
        raise Refused(f"{audit_toml}: [advisories] ignore is not a list of ids")
    return ignore


def advisory_front_matter(db: Path, advisory: str) -> dict:
    """The TOML block at the head of the advisory's file in the database."""
    matches = sorted(db.glob(f"crates/*/{advisory}.md"))
    if not matches:
        raise Refused(f"{advisory}: not in the advisory database at {db}")
    text = matches[0].read_text(encoding="utf-8")
    block = re.match(r"\s*```toml\n(.*?)\n```", text, re.S)
    if not block:
        raise Refused(f"{matches[0]}: no TOML front matter")
    return tomllib.loads(block.group(1))


def classify(ignores: list[str], db: Path, table: dict) -> list[str]:
    """The refusals for the ignore list against the table and the database."""
    if not (db / "crates").is_dir():
        raise Refused(
            f"no advisory database at {db}: run `cargo audit` first (it fetches one), "
            "or pass --advisory-db"
        )
    refusals = []
    for advisory in ignores:
        front = advisory_front_matter(db, advisory)
        if front.get("advisory", {}).get("informational") == "unsound" and advisory not in table:
            refusals.append(
                f"{advisory} is an unsound advisory ignored in {AUDIT_TOML.relative_to(REPO)} "
                "with no row in this script's TABLE: name the crate and the symbols that "
                "reach the bug, so the ignore is checked"
            )
    for advisory in table:
        if advisory not in ignores:
            refusals.append(f"TABLE row {advisory} is not ignored in audit.toml: a stale row")
    return refusals


def resolved_packages(tauri: Path) -> list[tuple[str, Path]]:
    """(name, source directory) of every package `cargo metadata` resolves."""
    fetch = subprocess.run(["cargo", "fetch", "--locked"], cwd=tauri, check=False)
    if fetch.returncode != 0:
        raise Refused(f"`cargo fetch --locked` in {tauri} failed ({fetch.returncode})")
    meta = subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1"],
        cwd=tauri,
        capture_output=True,
        text=True,
        check=False,
    )
    if meta.returncode != 0:
        raise Refused(f"`cargo metadata --locked` failed ({meta.returncode}): {meta.stderr.strip()}")
    data = json.loads(meta.stdout)
    return [(pkg["name"], Path(pkg["manifest_path"]).parent) for pkg in data["packages"]]


def scan(
    packages: list[tuple[str, Path]], own_source: Path, crate: str, symbols: tuple[str, ...]
) -> tuple[list[str], int]:
    """Matches as `file:line: text`, and how many packages were scanned."""
    if not any(name == crate for name, _ in packages):
        raise Refused(f"{crate} is not in the graph: the row has no subject (remove it with the ignore)")
    pattern = re.compile(r"\b(?:" + "|".join(re.escape(s) for s in symbols) + r")\b")
    roots = sorted({path for name, path in packages if name != crate} | {own_source})
    hits = []
    scanned = 0
    for root in roots:
        if not root.is_dir():
            continue
        scanned += 1
        for dirpath, _dirnames, filenames in os.walk(root):
            for filename in filenames:
                if not filename.endswith(SOURCE_SUFFIXES):
                    continue
                path = Path(dirpath) / filename
                try:
                    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
                except OSError as why:
                    raise Refused(f"{path}: unreadable ({why})") from None
                for number, line in enumerate(lines, 1):
                    if pattern.search(line):
                        hits.append(f"{path}:{number}: {line.strip()}")
    if scanned == 0:
        raise Refused(f"no package source was scanned for {crate}: a scan of nothing does not pass")
    return hits, scanned


def check(db: Path) -> int:
    failures = 0
    try:
        refusals = classify(ignored_ids(AUDIT_TOML), db, TABLE)
        for refusal in refusals:
            print(f"FAIL {refusal}")
            failures += 1
        packages = resolved_packages(TAURI)
        for advisory, (crate, symbols) in TABLE.items():
            hits, scanned = scan(packages, OWN_SOURCE, crate, symbols)
            for hit in hits:
                print(f"FAIL {advisory} ({crate}): {hit}")
            failures += len(hits)
            print(
                f"{advisory} ({crate}): {scanned} package sources and src-tauri/src scanned "
                f"for {', '.join(symbols)}; {len(hits)} match(es)"
            )
    except Refused as why:
        print(f"FAIL {why}")
        return 1
    print("unreached advisories: " + ("passed" if failures == 0 else f"{failures} failure(s)"))
    return 1 if failures else 0


def selftest() -> int:
    failures = 0

    def expect(label: str, ok: bool) -> None:
        nonlocal failures
        print(f"{'ok  ' if ok else 'FAIL'} {label}")
        failures += 0 if ok else 1

    def refused(call) -> bool:
        try:
            call()
        except Refused:
            return True
        return False

    symbols = ("array_iter_str", "VariantStrIter")
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        glib = root / "glib"
        (glib / "src").mkdir(parents=True)
        (glib / "src" / "variant.rs").write_text("pub struct VariantStrIter;\nfn array_iter_str() {}\n")
        clean = root / "gtk"
        (clean / "src").mkdir(parents=True)
        (clean / "src" / "lib.rs").write_text("fn main() { let x = 1; }\n")
        own = root / "own"
        own.mkdir()
        (own / "main.rs").write_text("fn run() {}\n")
        graph = [("glib", glib), ("gtk", clean)]

        hits, scanned = scan(graph, own, "glib", symbols)
        expect("a graph with no caller passes, glib's own source unread", hits == [] and scanned == 2)

        caller = root / "wry"
        (caller / "src").mkdir(parents=True)
        (caller / "src" / "menu.rs").write_text("use glib::VariantStrIter;\nlet _ = v.array_iter_str();\n")
        hits, _ = scan(graph + [("wry", caller)], own, "glib", symbols)
        expect(
            "a planted caller fails, named by file and line",
            len(hits) == 2 and hits[0].endswith("menu.rs:1: use glib::VariantStrIter;"),
        )

        (own / "tray.rs").write_text("for s in v.array_iter_str() {}\n")
        hits, _ = scan(graph, own, "glib", symbols)
        expect("a caller in the wallet's own source fails", len(hits) == 1)
        (own / "tray.rs").unlink()

        (clean / "src" / "lib.rs").write_text("let VariantStrIterator = 1;\n")
        hits, _ = scan(graph, own, "glib", symbols)
        expect("a longer identifier is not a match", hits == [])

        expect("an empty graph fails", refused(lambda: scan([], own, "glib", symbols)))
        expect(
            "a graph without the named crate fails",
            refused(lambda: scan([("gtk", clean)], own, "glib", symbols)),
        )
        gone = root / "missing"
        expect(
            "zero package sources scanned fails",
            refused(lambda: scan([("glib", glib), ("gtk", gone)], gone, "glib", symbols)),
        )

        db = root / "db"
        for crate, advisory, informational in (
            ("glib", "RUSTSEC-2024-0429", 'informational = "unsound"\n'),
            ("crypto-bigint", "RUSTSEC-2026-0387", ""),
            ("other", "RUSTSEC-2099-0001", 'informational = "unsound"\n'),
        ):
            (db / "crates" / crate).mkdir(parents=True)
            (db / "crates" / crate / f"{advisory}.md").write_text(
                f'```toml\n[advisory]\nid = "{advisory}"\npackage = "{crate}"\n{informational}```\n\n# Title\n'
            )
        table = {"RUSTSEC-2024-0429": ("glib", symbols)}
        expect(
            "an unsound ignore with a row and a vulnerability ignore pass",
            classify(["RUSTSEC-2026-0387", "RUSTSEC-2024-0429"], db, table) == [],
        )
        expect(
            "an unsound ignore with no row is refused",
            len(classify(["RUSTSEC-2024-0429", "RUSTSEC-2099-0001"], db, table)) == 1,
        )
        expect("a row whose advisory is not ignored is stale", len(classify([], db, table)) == 1)
        expect(
            "an ignored id the database does not hold is refused",
            refused(lambda: classify(["RUSTSEC-2000-0000"], db, table)),
        )
        expect("a missing advisory database is refused", refused(lambda: classify([], root / "nodb", table)))

    expect(
        "the committed audit.toml ignores every TABLE advisory",
        all(advisory in ignored_ids(AUDIT_TOML) for advisory in TABLE),
    )
    print("selftest: " + ("passed" if failures == 0 else f"{failures} failure(s)"))
    return 1 if failures else 0


def main(argv: list[str]) -> int:
    if argv == ["--selftest"]:
        return selftest()
    db = Path(os.environ.get("CARGO_HOME") or Path.home() / ".cargo") / "advisory-db"
    if argv[:1] == ["--advisory-db"] and len(argv) == 2:
        db = Path(argv[1])
    elif argv:
        print(__doc__ or "usage: check_unreached_advisories.py [--advisory-db DIR] | --selftest", file=sys.stderr)
        return 2
    return check(db)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
