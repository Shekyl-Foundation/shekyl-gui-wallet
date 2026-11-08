#!/usr/bin/env python3
# Copyright (c) 2026, The Shekyl Foundation
#
# All rights reserved.
# BSD-3-Clause
"""Offline tests for core_archive.py: every check that refuses, refuses.

The network legs (gh) are exercised by the release dry run; these tests
cover what the script decides from bytes it already holds. The signature
tests make throwaway keys and skip when gpg is absent.
"""

import io
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import core_archive as ca  # noqa: E402

SHA = "df9910ecb81a7f8821b26ba37bad0642d001f866"


def tar_bz2(path, members):
    """members: {name: (bytes, mode)}; directories are implied."""
    with tarfile.open(path, "w:bz2") as t:
        for name, (data, mode) in members.items():
            info = tarfile.TarInfo(name)
            info.size = len(data)
            info.mode = mode
            t.addfile(info, io.BytesIO(data))


def linux_members(top="shekyl-x86_64-linux-gnu-df9910ecb"):
    return {
        f"{top}/shekyld": (b"daemon", 0o755),
        f"{top}/tor/tor": (b"tor", 0o755),
        f"{top}/tor/libevent-2.1.so.7": (b"lib", 0o644),
        f"{top}/tor-licenses/tor.txt": (b"licence", 0o644),
    }


class TmpCase(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())

    def tearDown(self):
        shutil.rmtree(self.tmp)


class RunTests(unittest.TestCase):
    def good(self):
        return {"path": ca.GITIAN_WORKFLOW, "conclusion": "success", "head_sha": SHA}

    def test_a_passed_gitian_run_gives_its_commit(self):
        self.assertEqual(ca.check_run(self.good(), 1), SHA)

    def test_another_workflow_is_refused(self):
        run = dict(self.good(), path=".github/workflows/ci.yml")
        with self.assertRaisesRegex(ca.Refused, "not core's"):
            ca.check_run(run, 1)

    def test_a_failed_run_is_refused(self):
        run = dict(self.good(), conclusion="failure")
        with self.assertRaisesRegex(ca.Refused, "not success"):
            ca.check_run(run, 1)

    def test_an_artifact_without_a_sha256_digest_is_refused(self):
        with self.assertRaisesRegex(ca.Refused, "no sha256 digest"):
            ca.artifact_digest({"name": "gitian-Linux", "digest": None})
        self.assertEqual(ca.artifact_digest({"digest": "sha256:" + "a" * 64}), "a" * 64)


class ManifestTests(TmpCase):
    def write(self, text):
        p = self.tmp / ca.MANIFEST
        p.write_text(text, encoding="ascii")
        return p

    def test_the_line_for_the_name_gives_its_digest(self):
        m = self.write(f"{'a' * 64}  shekyl-x.zip\n{'b' * 64}  other\n")
        self.assertEqual(ca.manifest_digest(m, "shekyl-x.zip"), "a" * 64)

    def test_a_missing_line_is_refused(self):
        m = self.write(f"{'b' * 64}  other\n")
        with self.assertRaisesRegex(ca.Refused, "0 line"):
            ca.manifest_digest(m, "shekyl-x.zip")

    def test_a_duplicate_line_is_refused(self):
        m = self.write(f"{'a' * 64}  x\n{'b' * 64}  x\n")
        with self.assertRaisesRegex(ca.Refused, "2 line"):
            ca.manifest_digest(m, "x")

    def test_a_malformed_line_is_refused(self):
        m = self.write("not a digest line\n")
        with self.assertRaisesRegex(ca.Refused, "malformed"):
            ca.manifest_digest(m, "x")


class UnpackPlaceTests(TmpCase):
    def test_an_archive_is_placed_for_tauri_with_tor_executable(self):
        archives = self.tmp / "archives"
        archives.mkdir()
        tar_bz2(archives / "shekyl-x86_64-linux-gnu-df9910ecb.tar.bz2", linux_members())
        bins = self.tmp / "binaries"
        args = type("A", (), dict(
            archives=archives, host="x86_64-linux-gnu",
            tauri_triple="x86_64-unknown-linux-gnu", binaries=bins,
        ))
        ca.place_cmd(args)
        self.assertEqual((bins / "shekyld-x86_64-unknown-linux-gnu").read_bytes(), b"daemon")
        self.assertEqual(sorted(p.name for p in (bins / "tor").iterdir()), ["libevent-2.1.so.7", "tor"])
        if os.name != "nt":
            self.assertTrue(os.stat(bins / "tor" / "tor").st_mode & 0o100)

    def test_placing_over_an_existing_tor_is_refused(self):
        root = self.tmp / "root"
        for d in ("tor", "tor-licenses"):
            (root / d).mkdir(parents=True)
        (root / "shekyld").write_bytes(b"d")
        bins = self.tmp / "binaries"
        (bins / "tor").mkdir(parents=True)
        with self.assertRaisesRegex(ca.Refused, "clean tree"):
            ca.place(root, "x86_64-linux-gnu", "t", bins)

    def test_an_archive_without_tor_is_refused(self):
        root = self.tmp / "root"
        (root / "tor-licenses").mkdir(parents=True)
        (root / "shekyld").write_bytes(b"d")
        with self.assertRaisesRegex(ca.Refused, "no tor/"):
            ca.place(root, "x86_64-linux-gnu", "t", self.tmp / "b")

    def test_the_windows_sidecar_takes_the_exe(self):
        root = self.tmp / "root"
        for d in ("tor", "tor-licenses"):
            (root / d).mkdir(parents=True)
        (root / "shekyld.exe").write_bytes(b"pe")
        out = ca.place(root, "x86_64-w64-mingw32", "x86_64-pc-windows-msvc", self.tmp / "b")
        self.assertEqual(out.name, "shekyld-x86_64-pc-windows-msvc.exe")

    def test_two_top_level_entries_are_refused(self):
        a = self.tmp / "shekyl-x86_64-linux-gnu-x.tar.bz2"
        members = linux_members()
        members["stray"] = (b"x", 0o644)
        tar_bz2(a, members)
        with self.assertRaisesRegex(ca.Refused, "one top-level directory"):
            ca.unpack(a, self.tmp / "u")

    def test_a_zip_member_leaving_the_directory_is_refused(self):
        a = self.tmp / "shekyl-x86_64-w64-mingw32-x.zip"
        with zipfile.ZipFile(a, "w") as z:
            z.writestr("../evil", b"x")
        with self.assertRaisesRegex(ca.Refused, "unsafe member"):
            ca.unpack(a, self.tmp / "u")

    def test_two_archives_for_one_host_are_refused(self):
        for suffix in ("a", "b"):
            (self.tmp / f"shekyl-x86_64-linux-gnu-{suffix}.tar.bz2").write_bytes(b"")
        with self.assertRaisesRegex(ca.Refused, "2 archive"):
            ca.archive_for(self.tmp, "x86_64-linux-gnu")


def fake_runtime():
    """A 64-byte ELF64 header with no section headers: the runtime ends at 64."""
    head = bytearray(64)
    head[:6] = b"\x7fELF\x02\x01"
    head[0x28:0x30] = (64).to_bytes(8, "little")
    return bytes(head)


class ElfTests(unittest.TestCase):
    def test_the_runtime_ends_after_its_section_headers(self):
        head = bytearray(fake_runtime())
        head[0x28:0x30] = (1000).to_bytes(8, "little")
        head[0x3A:0x3C] = (64).to_bytes(2, "little")
        head[0x3C:0x3E] = (3).to_bytes(2, "little")
        self.assertEqual(ca.elf_end(bytes(head)), 1000 + 64 * 3)

    def test_a_file_that_is_not_elf64_is_refused(self):
        with self.assertRaisesRegex(ca.Refused, "not a 64-bit"):
            ca.elf_end(b"MZ" + bytes(62))


@unittest.skipUnless(shutil.which("mksquashfs") and shutil.which("unsquashfs"), "squashfs-tools is not installed")
class AppImageTests(TmpCase):
    def setUp(self):
        super().setUp()
        root = self.tmp / "root"
        (root / "usr" / "bin").mkdir(parents=True)
        (root / "usr" / "bin" / "app").write_bytes(b"app")
        image = self.tmp / "image.sqfs"
        subprocess.run(["mksquashfs", str(root), str(image), "-quiet", "-comp", "gzip"],
                       check=True, capture_output=True)
        self.appimage = self.tmp / "app.AppImage"
        self.appimage.write_bytes(fake_runtime() + image.read_bytes())
        self.bins = self.tmp / "binaries"
        (self.bins / "tor").mkdir(parents=True)
        (self.bins / "tor" / "tor").write_bytes(b"pinned tor")
        (self.bins / "tor" / "tor").chmod(0o755)
        (self.bins / "tor-licenses").mkdir()
        (self.bins / "tor-licenses" / "tor.txt").write_bytes(b"licence")
        self.conf = self.tmp / "tauri.conf.json"
        self.conf.write_text('{"productName": "Shekyl Wallet"}', encoding="utf-8")

    def args(self, **kw):
        return type("A", (), dict(appimage=self.appimage, binaries=self.bins, tauri_conf=self.conf, **kw))

    def test_tor_goes_in_under_the_resource_directory_with_the_runtime_kept(self):
        ca.appimage_add_tor(self.args())
        self.assertEqual(self.appimage.read_bytes()[:64], fake_runtime())
        out = self.tmp / "out"
        ca.appimage_unpack(type("A", (), dict(appimage=self.appimage, dest=out)))
        tor = out / "usr" / "lib" / "Shekyl Wallet" / "tor" / "tor"
        self.assertEqual(tor.read_bytes(), b"pinned tor")
        self.assertTrue(os.stat(tor).st_mode & 0o100)
        self.assertEqual((out / "usr" / "bin" / "app").read_bytes(), b"app")
        self.assertTrue((out / "usr" / "lib" / "Shekyl Wallet" / "tor-licenses" / "tor.txt").is_file())

    def test_an_appimage_that_already_holds_tor_is_refused(self):
        ca.appimage_add_tor(self.args())
        with self.assertRaisesRegex(ca.Refused, "already holds tor/"):
            ca.appimage_add_tor(self.args())

    def test_an_appimage_with_no_squashfs_after_its_runtime_is_refused(self):
        self.appimage.write_bytes(fake_runtime() + b"not a squashfs")
        with self.assertRaisesRegex(ca.Refused, "no squashfs"):
            ca.appimage_add_tor(self.args())


@unittest.skipUnless(shutil.which("gpg"), "gpg is not installed")
class SignatureTests(TmpCase):
    def key(self, home, name):
        env = dict(os.environ, GNUPGHOME=str(home))
        subprocess.run(
            ["gpg", "--batch", "--pinentry-mode", "loopback", "--passphrase", "",
             "--quick-gen-key", name, "ed25519", "sign", "1d"],
            env=env, check=True, capture_output=True,
        )
        out = subprocess.run(
            ["gpg", "--batch", "--with-colons", "--list-keys", name],
            env=env, check=True, capture_output=True, text=True,
        ).stdout
        fpr = next(ln.split(":")[9] for ln in out.splitlines() if ln.startswith("fpr:"))
        block = subprocess.run(
            ["gpg", "--batch", "--armor", "--export", name],
            env=env, check=True, capture_output=True, text=True,
        ).stdout
        return env, fpr, block

    def setUp(self):
        super().setUp()
        home = self.tmp / "gnupg"
        home.mkdir(mode=0o700)
        self.env, self.fpr, self.block = self.key(home, "signer@example.invalid")
        self.manifest = self.tmp / ca.MANIFEST
        self.manifest.write_text(f"{'a' * 64}  x\n", encoding="ascii")
        self.signature = self.tmp / ca.SIGNATURE
        subprocess.run(
            ["gpg", "--batch", "--pinentry-mode", "loopback", "--passphrase", "", "--armor",
             "--detach-sign", "--output", str(self.signature), str(self.manifest)],
            env=self.env, check=True, capture_output=True,
        )

    def tearDown(self):
        subprocess.run(["gpgconf", "--kill", "gpg-agent"], env=self.env, capture_output=True)
        super().tearDown()

    def test_the_pinned_signer_verifies(self):
        ca.check_manifest_signature(self.manifest, self.signature, self.block, self.fpr)

    def test_a_signature_by_another_key_is_refused(self):
        with self.assertRaisesRegex(ca.Refused, "not the pinned subkey"):
            ca.check_manifest_signature(self.manifest, self.signature, self.block, "F" * 40)

    def test_an_edited_manifest_is_refused(self):
        self.manifest.write_text(f"{'b' * 64}  x\n", encoding="ascii")
        with self.assertRaisesRegex(ca.Refused, "does not verify"):
            ca.check_manifest_signature(self.manifest, self.signature, self.block, self.fpr)


if __name__ == "__main__":
    unittest.main()
