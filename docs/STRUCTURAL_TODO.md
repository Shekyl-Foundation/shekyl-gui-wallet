# Structural TODO

Long-term improvements that are not blocking current releases but should be
addressed as the project matures.

## ~~Static linking of third-party libraries (Linux)~~ DONE

**Status**: Implemented via `contrib/depends` integration.

The Linux release build now uses shekyl-core's `contrib/depends` system to build
all third-party libraries (Boost, OpenSSL, libsodium, protobuf, libunbound,
hidapi, libusb, zeromq) from source with static linking. The `.deb` only depends
on system-level packages (`libwebkit2gtk-4.1-0`, `libayatana-appindicator3-1`,
`libudev1`). A single `.deb` works across Ubuntu 22.04, 24.04, and other distros.

## Use `contrib/depends` for macOS builds

**Priority**: Low
**Prerequisite**: Darwin SDK available on GitHub Actions macOS runners

The `contrib/depends` system supports macOS cross-compilation
(`HOST=aarch64-apple-darwin11`). Using it would replace Homebrew in the macOS CI
build, giving identical static linking and eliminating the Homebrew prefix
detection in `build.rs`. Currently deferred because Homebrew works and the darwin
SDK setup for depends is non-trivial.

## Replace Boost with Rust crates (long-term)

**Priority**: Low (packaging problem solved by static linking)
**Tracking**: Happens organically as wallet2 code migrates to Rust

### Migration tiers

**Tier 1 -- Easy wins (weeks):** `boost::optional`/`variant` (Rust built-in),
`program_options` (clap), `regex` (regex crate), `chrono`/`date_time` (chrono
crate), `algorithm` (str methods), `format` (format! macro).

**Tier 2 -- Medium (1-2 months):** `filesystem` (std::fs), `thread`
(std::thread/sync/parking_lot).

**Tier 3 -- Hard (1-2 months + format migration):** `serialization` (serde +
bincode; requires wallet cache format migration).

**Tier 4 -- Hardest (2-3 months):** `asio` (tokio; entire epee networking
rewrite).

## Replace epee (parallel track)

**Priority**: Medium
**Tracking**: Separate initiative from Boost replacement

Epee provides: custom HTTP stack, `portable_storage` serialization, logging
(`MINFO`/`MERROR`), `string_tools`, `wipeable_string`, `span`, `file_io_utils`.

Analysis shows ~30% of Boost usage is inside epee and ~70% is direct usage in
`src/`. Replacing epee eliminates its Boost subset but does not shortcut the
direct Boost usage in wallet2/core. Both tracks must complete independently.

Rust replacements: `hyper`/`axum` (HTTP), `serde` (serialization), `tracing`
(logging), std Rust equivalents for utilities.
