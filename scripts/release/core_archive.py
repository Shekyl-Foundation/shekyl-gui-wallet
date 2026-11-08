#!/usr/bin/env python3
# Copyright (c) 2026, The Shekyl Foundation
#
# All rights reserved.
# BSD-3-Clause
"""Stage shekyl-core's gitian archive for one installer (GUI_SYNC_LANE.md GS-1).

The installers carry core's gitian-built `shekyld` and its pinned Tor, never
a rebuild on the runner (GS-Q5). This script fetches core's archive for one
gitian host, checks its digest before unpacking, and places three things
for the Tauri build:

  binaries/shekyld-<tauri triple>[.exe]   the sidecar (`externalBin`)
  binaries/tor/                           Tor, exactly the pinned files (TB-7)
  binaries/tor-licenses/                  Tor's licences

An archive comes from one of two sources, and each has its own digest check:

  release TAG  core's published release. `SHA256SUMS.asc` must verify under
               the Foundation signing subkey pinned below, and the archive
               must match its line in `SHA256SUMS`.
  run ID       a core gitian dry run, before the tag exists. The run must
               be core's gitian workflow and must have succeeded, and the
               downloaded artifact must match the digest GitHub recorded
               for it. A dry run's archive is rehearsed, never shipped: a
               tag build always takes the release.

Commands:

  run-commit --run ID      print the core commit a gitian run built, after
                           checking the run. The release job checks out that
                           commit for the Engine crates.
  fetch ... --out DIR      fetch the archive for each named host and check
                           its digest. One Linux job runs this for every
                           installer, so the signature check has one home.
  place --archive A ...    unpack a checked archive and place it for Tauri.
  appimage-unpack ...      unpack an AppImage's files without running it.
  appimage-add-tor ...     put the placed tor/ and tor-licenses/ into a built
                           AppImage. Tauri's AppImage step runs linuxdeploy
                           over every ELF file in the AppDir, and it re-paths
                           Tor's, which the pin then refuses; so the AppImage
                           is built without Tor and Tor goes in afterwards.
  check-installed ...      run a bundled `shekyld --config-verify` against
                           the bundled Tor, and refuse if the daemon cannot
                           pin it.

Exit codes: 0 done; 1 refused (the message says why); 2 usage error.
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path

CORE_REPO = "Shekyl-Foundation/shekyl-core"
GITIAN_WORKFLOW = ".github/workflows/gitian.yml"

# The Foundation's release signing subkey. Core's docs/SIGNING.md is
# canonical, and scripts/release/sign_release_assets.py holds the same
# value. A rotation changes this line in the PR that pairs the GUI with
# the first core release signed by the new subkey.
FOUNDATION_SIGNING_SUBKEY_FPR = "3778B4C863C61512B5FC22036914D74823DDA8DC"

MANIFEST = "SHA256SUMS"
SIGNATURE = "SHA256SUMS.asc"

# Gitian hosts the installers build, with the artifact each gitian run
# uploads them in and the archive's extension.
HOSTS = {
    "x86_64-linux-gnu": ("Linux", ".tar.bz2"),
    "x86_64-w64-mingw32": ("Windows", ".zip"),
    "aarch64-apple-darwin11": ("macOS", ".tar.bz2"),
}

# A gitian dry run names its archive by the commit's first nine hex digits.
RUN_SUFFIX_LEN = 9


class Refused(Exception):
    pass


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def run(argv, **kwargs):
    """Run a command; its verdict is its own exit status (never a pipe)."""
    return subprocess.run(argv, capture_output=True, text=True, **kwargs)


def gh_json(path):
    proc = run(["gh", "api", path])
    if proc.returncode != 0:
        raise Refused(f"`gh api {path}` failed: {proc.stderr.strip()[:300]}")
    return json.loads(proc.stdout)


# ── Sources ────────────────────────────────────────────────────────────────


def check_run(run_json, run_id):
    """A gitian run is usable when it is core's gitian workflow and passed."""
    if run_json.get("path") != GITIAN_WORKFLOW:
        raise Refused(
            f"run {run_id} is {run_json.get('path')!r}, not core's {GITIAN_WORKFLOW}"
        )
    if run_json.get("conclusion") != "success":
        raise Refused(
            f"run {run_id} concluded {run_json.get('conclusion')!r}, not success"
        )
    sha = run_json.get("head_sha", "")
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise Refused(f"run {run_id} has no commit: {sha!r}")
    return sha


def run_commit(run_id):
    return check_run(gh_json(f"repos/{CORE_REPO}/actions/runs/{run_id}"), run_id)


def artifact_digest(artifact):
    """GitHub records `sha256:<hex>` for an artifact's zip."""
    digest = artifact.get("digest") or ""
    m = re.fullmatch(r"sha256:([0-9a-f]{64})", digest)
    if not m:
        raise Refused(
            f"artifact {artifact.get('name')!r} has no sha256 digest ({digest!r})"
        )
    return m.group(1)


def fetch_from_run(run_id, host, work):
    """Download the run's artifact for `host`, check its digest, and return
    the archive inside it."""
    sha = run_commit(run_id)
    artifact_name, ext = HOSTS[host]
    artifact_name = f"gitian-{artifact_name}"
    listing = gh_json(
        f"repos/{CORE_REPO}/actions/runs/{run_id}/artifacts?per_page=100"
    )
    matches = [a for a in listing.get("artifacts", []) if a.get("name") == artifact_name]
    if len(matches) != 1:
        raise Refused(f"run {run_id} has {len(matches)} artifact(s) named {artifact_name}")
    artifact = matches[0]
    if artifact.get("expired"):
        raise Refused(f"run {run_id}'s {artifact_name} has expired")
    want = artifact_digest(artifact)
    zipped = work / f"{artifact_name}.zip"
    with open(zipped, "wb") as out:
        proc = subprocess.run(
            ["gh", "api", f"repos/{CORE_REPO}/actions/artifacts/{artifact['id']}/zip"],
            stdout=out,
            stderr=subprocess.PIPE,
            text=False,
        )
    if proc.returncode != 0:
        raise Refused(f"downloading {artifact_name} failed: {proc.stderr.decode()[:300]}")
    have = sha256_file(zipped)
    if have != want:
        raise Refused(f"{artifact_name}: GitHub recorded {want}, the download is {have}")
    name = f"shekyl-{host}-{sha[:RUN_SUFFIX_LEN]}{ext}"
    unzipped = work / artifact_name
    with zipfile.ZipFile(zipped) as z:
        if name not in z.namelist():
            raise Refused(f"{artifact_name} of run {run_id} holds no {name}")
        z.extract(name, unzipped)
    return unzipped / name


def foundation_key_block(core):
    """The Foundation's public key, as core's SIGNING.md publishes it."""
    text = (core / "docs" / "SIGNING.md").read_text(encoding="utf-8")
    m = re.search(
        r"-----BEGIN PGP PUBLIC KEY BLOCK-----.*?-----END PGP PUBLIC KEY BLOCK-----",
        text,
        re.S,
    )
    if not m:
        raise Refused(f"{core}/docs/SIGNING.md publishes no public key block")
    return m.group(0) + "\n"


def check_manifest_signature(manifest, signature, key_block, expected_fpr):
    """The manifest verifies under exactly the pinned subkey."""
    with tempfile.TemporaryDirectory() as home:
        os.chmod(home, 0o700)
        env = dict(os.environ, GNUPGHOME=home)
        proc = run(["gpg", "--batch", "--import"], input=key_block, env=env)
        if proc.returncode != 0:
            raise Refused(f"importing the Foundation key failed: {proc.stderr.strip()[:300]}")
        proc = run(
            ["gpg", "--batch", "--status-fd", "1", "--verify", str(signature), str(manifest)],
            env=env,
        )
        if proc.returncode != 0:
            raise Refused(f"{MANIFEST} does not verify: {proc.stderr.strip()[:300]}")
        m = re.search(r"^\[GNUPG:\] VALIDSIG ([0-9A-F]{40})", proc.stdout, re.M)
        got = m.group(1) if m else "<none>"
        if got != expected_fpr:
            raise Refused(f"{MANIFEST} is signed by {got}, not the pinned subkey {expected_fpr}")


def manifest_digest(manifest, name):
    """The digest the manifest gives `name`: exactly one well-formed line."""
    found = []
    for line in manifest.read_text(encoding="ascii").splitlines():
        m = re.fullmatch(r"([0-9a-f]{64})  (\S.*)", line)
        if not m:
            raise Refused(f"malformed {MANIFEST} line: {line!r}")
        if m.group(2) == name:
            found.append(m.group(1))
    if len(found) != 1:
        raise Refused(f"{MANIFEST} has {len(found)} line(s) for {name}")
    return found[0]


def fetch_from_release(tag, host, work, core):
    _, ext = HOSTS[host]
    name = f"shekyl-{host}-{tag}{ext}"
    proc = run(
        ["gh", "release", "download", tag, "-R", CORE_REPO, "--dir", str(work),
         "-p", MANIFEST, "-p", SIGNATURE, "-p", name]
    )
    if proc.returncode != 0:
        raise Refused(f"`gh release download {tag}` failed: {proc.stderr.strip()[:300]}")
    manifest, signature, archive = work / MANIFEST, work / SIGNATURE, work / name
    for p in (manifest, signature, archive):
        if not p.is_file():
            raise Refused(f"core release {tag} has no {p.name}")
    check_manifest_signature(
        manifest, signature, foundation_key_block(core), FOUNDATION_SIGNING_SUBKEY_FPR
    )
    want = manifest_digest(manifest, name)
    have = sha256_file(archive)
    if have != want:
        raise Refused(f"{name}: {MANIFEST} gives {want}, the download is {have}")
    return archive


# ── Unpack and place ───────────────────────────────────────────────────────


def unpack(archive, dest):
    """Unpack into `dest` and return the archive's one top-level directory."""
    if archive.name.endswith(".zip"):
        with zipfile.ZipFile(archive) as z:
            for n in z.namelist():
                parts = Path(n).parts
                if Path(n).is_absolute() or ".." in parts:
                    raise Refused(f"{archive.name} has an unsafe member {n!r}")
            z.extractall(dest)
    else:
        with tarfile.open(archive, "r:bz2") as t:
            t.extractall(dest, filter="data")
    tops = sorted(dest.iterdir())
    if len(tops) != 1 or not tops[0].is_dir():
        raise Refused(f"{archive.name} must hold one top-level directory, found {[p.name for p in tops]}")
    return tops[0]


def place(root, host, tauri_triple, binaries):
    """Copy the sidecar, tor/ and tor-licenses/ into `binaries`."""
    windows = "mingw32" in host
    exe = ".exe" if windows else ""
    daemon = root / f"shekyld{exe}"
    if not daemon.is_file():
        raise Refused(f"the archive has no shekyld{exe}")
    for d in ("tor", "tor-licenses"):
        if not (root / d).is_dir():
            raise Refused(f"the archive has no {d}/")
        if (binaries / d).exists():
            raise Refused(f"{binaries / d} already exists; stage into a clean tree")
    binaries.mkdir(parents=True, exist_ok=True)
    sidecar = binaries / f"shekyld-{tauri_triple}{exe}"
    shutil.copyfile(daemon, sidecar)
    sidecar.chmod(0o755)
    # copytree keeps each file's mode, so tor keeps its executable bit.
    shutil.copytree(root / "tor", binaries / "tor")
    shutil.copytree(root / "tor-licenses", binaries / "tor-licenses")
    return sidecar


def fetch(args):
    args.out.mkdir(parents=True, exist_ok=True)
    for host in args.host:
        work = Path(tempfile.mkdtemp(prefix="core-archive-"))
        if args.release:
            archive = fetch_from_release(args.release, host, work, args.core)
            source = f"release {args.release}"
        else:
            archive = fetch_from_run(args.run, host, work)
            source = f"gitian run {args.run}"
        out = args.out / archive.name
        if out.exists():
            raise Refused(f"{out} already exists; fetch into a clean directory")
        shutil.copyfile(archive, out)
        print(f"core_archive: {archive.name} from {source}, sha256 {sha256_file(out)}")


def archive_for(archives, host):
    """The one archive in `archives` for `host`."""
    _, ext = HOSTS[host]
    found = sorted(p for p in archives.iterdir() if p.name.startswith(f"shekyl-{host}-") and p.name.endswith(ext))
    if len(found) != 1:
        raise Refused(f"{archives} holds {len(found)} archive(s) for {host}")
    return found[0]


def place_cmd(args):
    archive = archive_for(args.archives, args.host)
    work = Path(tempfile.mkdtemp(prefix="core-archive-"))
    root = unpack(archive, work)
    sidecar = place(root, args.host, args.tauri_triple, args.binaries)
    print(f"core_archive: placed {archive.name}")
    print(f"  sidecar  {sidecar}")
    for d in ("tor", "tor-licenses"):
        names = sorted(p.name for p in (args.binaries / d).iterdir())
        print(f"  {d + '/':14} {', '.join(names)}")


# ── The AppImage's Tor ─────────────────────────────────────────────────────


def elf_end(head):
    """Where a 64-bit little-endian ELF file ends: past its section headers.
    An AppImage's squashfs starts there, after the runtime."""
    if head[:4] != b"\x7fELF" or head[4] != 2 or head[5] != 1:
        raise Refused("the AppImage's runtime is not a 64-bit little-endian ELF file")
    shoff = int.from_bytes(head[0x28:0x30], "little")
    shentsize = int.from_bytes(head[0x3A:0x3C], "little")
    shnum = int.from_bytes(head[0x3C:0x3E], "little")
    return shoff + shentsize * shnum


def squashfs_settings(image):
    """The compression and block size `image` was made with."""
    proc = run(["unsquashfs", "-s", str(image)])
    if proc.returncode != 0:
        raise Refused(f"`unsquashfs -s` failed: {proc.stderr.strip()[:300]}")
    comp = re.search(r"^Compression (\S+)$", proc.stdout, re.M)
    block = re.search(r"^Block size (\d+)$", proc.stdout, re.M)
    if not comp or not block:
        raise Refused("`unsquashfs -s` names no compression or block size")
    return comp.group(1), block.group(1)


def appimage_image(appimage, work):
    """Write the squashfs behind `appimage`'s runtime to `work` and return
    (runtime bytes, image path). The AppImage is never run."""
    data = appimage.read_bytes()
    end = elf_end(data[:64])
    if data[end:end + 4] != b"hsqs":
        raise Refused(f"{appimage.name} has no squashfs after its runtime")
    image = work / "image.sqfs"
    image.write_bytes(data[end:])
    return data[:end], image


def unsquash(image, dest):
    proc = run(["unsquashfs", "-q", "-d", str(dest), str(image)])
    if proc.returncode != 0:
        raise Refused(f"`unsquashfs` failed: {proc.stderr.strip()[:300]}")


def appimage_unpack(args):
    work = Path(tempfile.mkdtemp(prefix="appimage-"))
    _, image = appimage_image(args.appimage, work)
    unsquash(image, args.dest)
    shutil.rmtree(work)


def appimage_add_tor(args):
    product = json.loads(args.tauri_conf.read_text(encoding="utf-8"))["productName"]
    work = Path(tempfile.mkdtemp(prefix="appimage-"))
    runtime, image = appimage_image(args.appimage, work)
    comp, block = squashfs_settings(image)
    root = work / "root"
    unsquash(image, root)
    # Tauri's resource directory on Linux: `../lib/<productName>` beside the
    # executable, as in the .deb.
    resources = root / "usr" / "lib" / product
    resources.mkdir(parents=True, exist_ok=True)
    for d in ("tor", "tor-licenses"):
        if (resources / d).exists():
            raise Refused(f"the AppImage already holds {d}/; build it without the release config")
        shutil.copytree(args.binaries / d, resources / d)
    rebuilt = work / "rebuilt.sqfs"
    proc = run([
        "mksquashfs", str(root), str(rebuilt), "-root-owned", "-noappend",
        "-comp", comp, "-b", block, "-quiet",
    ])
    if proc.returncode != 0:
        raise Refused(f"`mksquashfs` failed: {proc.stderr.strip()[:300]}")
    out = args.appimage.with_name(args.appimage.name + ".new")
    with open(out, "wb") as f:
        f.write(runtime)
        f.write(rebuilt.read_bytes())
    out.chmod(0o755)
    out.replace(args.appimage)
    shutil.rmtree(work)
    print(f"core_archive: added tor/ and tor-licenses/ to {args.appimage.name} "
          f"under usr/lib/{product}/ ({comp}, block {block})")


# ── The installed daemon pins the installed Tor ────────────────────────────


def check_installed(args):
    """`--config-verify` reads the managed Tor's pin without starting it; a
    file it cannot pin is a `warning: managed Tor:` line (TB-2)."""
    tor = args.tor_file
    if not tor.is_file():
        raise Refused(f"the bundle has no Tor at {tor}")
    if os.name != "nt" and not os.stat(tor).st_mode & stat.S_IXUSR:
        raise Refused(f"{tor} is not executable")
    with tempfile.TemporaryDirectory() as d:
        argv = [
            str(args.shekyld), "--config-verify",
            "--tor", "managed", "--tor-file", str(tor),
            "--data-dir", str(Path(d) / "data"),
            "--log-file", str(Path(d) / "shekyld.log"),
        ]
        proc = run(argv)
    out = proc.stdout + proc.stderr
    print(out.rstrip())
    if proc.returncode != 0:
        raise Refused(f"`shekyld --config-verify` exited {proc.returncode}")
    if "warning: managed Tor" in out:
        raise Refused("the bundled daemon cannot pin the bundled Tor")
    # The pin is the bytes; this is that they run where they were put, with
    # the libraries beside them.
    proc = run([str(tor), "--version"])
    print(proc.stdout.strip())
    if proc.returncode != 0 or not proc.stdout.startswith("Tor version"):
        raise Refused(f"the bundled Tor does not run: {(proc.stdout + proc.stderr).strip()[:300]}")


# ── Entry ──────────────────────────────────────────────────────────────────


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = parser.add_subparsers(dest="command", required=True)

    cmd = sub.add_parser("run-commit")
    cmd.add_argument("--run", required=True, type=int)

    cmd = sub.add_parser("fetch")
    src = cmd.add_mutually_exclusive_group(required=True)
    src.add_argument("--release", help="core release tag, e.g. v3.1.0-alpha.12")
    src.add_argument("--run", type=int, help="core gitian run id (a dry run)")
    cmd.add_argument("--host", required=True, action="append", choices=sorted(HOSTS))
    cmd.add_argument("--core", type=Path, help="core checkout at the tag (release only)")
    cmd.add_argument("--out", required=True, type=Path)

    cmd = sub.add_parser("place")
    cmd.add_argument("--archives", required=True, type=Path, help="fetch's output")
    cmd.add_argument("--host", required=True, choices=sorted(HOSTS))
    cmd.add_argument("--tauri-triple", required=True)
    cmd.add_argument("--binaries", required=True, type=Path)

    cmd = sub.add_parser("appimage-add-tor")
    cmd.add_argument("--appimage", required=True, type=Path)
    cmd.add_argument("--binaries", required=True, type=Path, help="place's output")
    cmd.add_argument("--tauri-conf", required=True, type=Path)

    cmd = sub.add_parser("appimage-unpack")
    cmd.add_argument("--appimage", required=True, type=Path)
    cmd.add_argument("--dest", required=True, type=Path)

    cmd = sub.add_parser("check-installed")
    cmd.add_argument("--shekyld", required=True, type=Path)
    cmd.add_argument("--tor-file", required=True, type=Path)

    args = parser.parse_args(argv)
    try:
        if args.command == "run-commit":
            print(run_commit(args.run))
        elif args.command == "fetch":
            if args.release and args.core is None:
                parser.error("fetch --release needs --core, for the Foundation key")
            fetch(args)
        elif args.command == "place":
            place_cmd(args)
        elif args.command == "appimage-unpack":
            appimage_unpack(args)
        elif args.command == "appimage-add-tor":
            appimage_add_tor(args)
        else:
            check_installed(args)
    except Refused as exc:
        print(f"core_archive: REFUSED: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
