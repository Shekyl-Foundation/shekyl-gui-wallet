# Structural TODO

Long-term improvements that are not blocking current releases but should be
addressed as the project matures.

## Static linking of third-party libraries (Linux)

**Priority**: Medium
**Tracking**: Option 3 from the multi-distro `.deb` discussion

### Problem

The Linux `.deb` packages depend on distro-specific versioned shared libraries
(e.g., `libboost-filesystem1.74.0` on Ubuntu 22.04, `libboost-filesystem1.83.0`
on 24.04). Each Ubuntu LTS ships its own Boost/protobuf/OpenSSL version and the
packages are not cross-installable. This forces us to build a separate `.deb`
per Ubuntu release and leaves non-Ubuntu distros without a `.deb` at all.

### Current workaround

- Build matrix includes both `ubuntu-22.04` and `ubuntu-24.04`, producing
  distro-suffixed `.deb` files (`_ubuntu-22.04.deb`, `_ubuntu-24.04.deb`).
- AppImage is offered as the universal Linux option (bundles all `.so` files).

### Target state

Statically link Boost, OpenSSL, libsodium, protobuf, libunbound, hidapi, and
libusb from source in CI so the resulting binary has zero third-party shared
library dependencies. This produces a single `.deb` that works on any Linux
distro without version-pinned dependencies.

### Why it's not done yet

Distro-provided static libraries (from `-dev` packages) are **not compiled with
`-fPIC`**, which is required when linking into Tauri's `cdylib` output. The
linker fails with:

```
relocation R_X86_64_PC32 cannot be used against symbol 'stderr';
recompile with -fPIC
```

### Implementation path

1. Add CI steps to build Boost, OpenSSL, libsodium, protobuf, libunbound,
   hidapi, and libusb **from source** with `-fPIC` (e.g., `./b2 cflags=-fPIC
   cxxflags=-fPIC` for Boost, `./Configure -fPIC` for OpenSSL).
2. Install these into a local prefix (e.g., `${{ runner.temp }}/static-deps`).
3. Point `build.rs` at that prefix via `SHEKYL_STATIC_DEPS` or similar env var.
4. Switch `build.rs` Linux section from `dylib=` to `static=` linking.
5. Remove distro-versioned entries from `tauri.conf.json` `.deb` depends
   (keep only truly system-level deps like `libudev1`, `libwebkit2gtk-4.1-0`).
6. Collapse the Ubuntu matrix back to a single runner.
7. Verify the resulting binary runs on Ubuntu 22.04, 24.04, Fedora, and Arch.

### Estimated effort

~1-2 days of CI pipeline work. Boost is the slowest to build from source
(~3-5 min with ccache). All other libraries are fast (<30s each).
