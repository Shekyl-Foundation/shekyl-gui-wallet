# Shekyl GUI Wallet Changelog

## [3.1.0-alpha.1] - 2026-04-15

First release under the unified Shekyl versioning scheme. The version
jumps from 0.4.0-beta.2 to 3.1.0-alpha.1 to align the software major
version with the Shekyl protocol version at first public release. This
is not a regression from beta to alpha — it is a re-baseline. The
pre-release stage reflects that the combined system (daemon + wallet) has
not yet completed stressnet validation. See `shekyl-core/docs/VERSIONING.md`
for the full versioning scheme.

Software versions follow SemVer independently per repo. The GUI wallet
and shekyl-core are not version-coupled; each declares which protocol
version it requires. This release requires `protocol_version = 3`.

### Added

- V3.1 multisig UI components: FingerprintBadge, ProverView,
  LossAcknowledgment, AddressProvenance, RelayConfig, ViolationAlert,
  SigningDashboard.
- File-based transport for multisig signing (first-class, equal
  prominence with relay transport).
- GroupDescriptor export/import for multisig group backup.
- Failure-mode UX: 6 alert banners for multisig operational failures.
- Address format discipline rule (`65-address-format-discipline.mdc`).

### Changed

- Version scheme: aligned to Shekyl protocol versioning (3.x series).
  All three version sources (package.json, src-tauri/Cargo.toml,
  tauri.conf.json) now report 3.1.0-alpha.1.

## [0.4.0-beta.2] - 2026-04-13

_Last release under the pre-alignment version scheme._
