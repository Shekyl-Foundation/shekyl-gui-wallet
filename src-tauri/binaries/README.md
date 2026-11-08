# Sidecar binaries and the bundled Tor

The release build places three things here from shekyl-core's gitian
archive for the target. Nothing in this directory is committed or built on
the runner: the installers carry core's gitian-built daemon and its pinned
Tor (`docs/design/GUI_SYNC_LANE.md` GS-Q5, GS-1).

| Path | What it is | How it ships |
|------|------------|--------------|
| `shekyld-<triple>[.exe]` | The Shekyl daemon | Tauri `externalBin` (`tauri.conf.json`) |
| `tor/` | Tor, exactly core's pinned files for the target | A resource directory (`tauri.release.conf.json`) |
| `tor-licenses/` | Tor's licences and source note | A resource directory (`tauri.release.conf.json`) |

The wallet itself is not a bundled binary: it runs in-process as
`shekyl-engine-core::Engine` (see `src-tauri/src/engine_session.rs`).

## How the release places them

`.github/workflows/release.yml`, job `core-archives`, fetches core's archive
for every installer with `scripts/release/core_archive.py fetch` and checks
its digest before anything is unpacked:

- **A tag build** takes core's release at `SHEKYL_CORE_REF`. Its
  `SHA256SUMS.asc` must verify under the Foundation signing subkey pinned in
  `core_archive.py`, and the archive must match its line in `SHA256SUMS`.
- **A dry run** (`workflow_dispatch`) takes a core gitian run by id,
  `core_gitian_run`. The run must be core's gitian workflow and must have
  passed, and each artifact must match the digest GitHub recorded. The
  Engine crates compile against the commit that run built.

Each build job then runs `core_archive.py place`, which writes:

```
shekyld-x86_64-unknown-linux-gnu        from shekyl-x86_64-linux-gnu-*.tar.bz2
shekyld-aarch64-apple-darwin            from shekyl-aarch64-apple-darwin11-*.tar.bz2
shekyld-x86_64-pc-windows-msvc.exe      from shekyl-x86_64-w64-mingw32-*.zip
tor/  tor-licenses/                     from the same archive
```

and checks the staged `tor/` against core's pins with
`tor_bundle.py verify`. On macOS, `tor/` holds the Foundation's Developer
ID-signed `tor` and `libevent` (core TB-11). Nothing here re-signs them:
re-signing changes the bytes, and the pin refuses them.

After the bundle is built, the job reads Tor back out of what ships (the
`.deb`, `.rpm` and AppImage payloads, the mounted `.dmg`, the installed NSIS
tree), verifies it against the pins again, and runs the bundled
`shekyld --config-verify` against it. The daemon reads the pin there
without starting Tor, so a Tor it cannot pin fails the release.

`tor/` stays out of `tauri.conf.json` so that dev builds and CI need no
Tor. Only the release passes `--config src-tauri/tauri.release.conf.json`.

The AppImage is the exception. Tauri's AppImage step runs `linuxdeploy`
over every ELF file in the AppDir, and it re-paths Tor's files, so their
bytes no longer match the pin. The release therefore builds the `.deb` and
`.rpm` with the release config, builds the AppImage without it, and then
runs `core_archive.py appimage-add-tor`. That command puts `tor/` and
`tor-licenses/` into the AppImage's squashfs under `usr/lib/<productName>/`,
the resource directory Tauri reads on Linux, and keeps the runtime as it
was. The after-bundle check unpacks the AppImage with
`core_archive.py appimage-unpack` rather than running it.

## At run time

`DaemonManager` (`src-tauri/src/daemon_manager.rs`) starts the sidecar with
`--tor-file` naming `tor/tor` (`tor\tor.exe` on Windows) under the app's
resource directory, one mechanism on every platform. The daemon checks that
file against its pin at every spawn. A file it cannot pin is a warning,
not a refusal: the node holds its own transactions until Tor is up
(core TB-2).

## Development

During `tauri dev` no sidecar is present. `DaemonManager` logs a warning,
sets the daemon mode to `Unavailable`, and the wallet uses whatever daemon
URL is configured in Settings. To try the release layout locally, run
`core_archive.py fetch` and `place` into this directory, then
`npx tauri build --config src-tauri/tauri.release.conf.json`.

Tauri needs a file at the sidecar's name before it compiles anything, so
CI creates an empty placeholder (`.github/workflows/ci.yml`). Check your
target triple with `rustc -vV | grep host`.

## Git

`.gitignore` excludes `shekyld-*`, `tor/` and `tor-licenses/` here.
