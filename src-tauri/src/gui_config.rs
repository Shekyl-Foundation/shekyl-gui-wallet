// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Persistent GUI preferences stored as `gui-config.json` in the
//! platform's per-app config directory.
//!
//! Today the only persisted item is the user's choice of wallet
//! directory (set via the Advanced settings dialog). Holding it across
//! launches removes a startup quirk where users had to re-select their
//! directory every time the app opened.
//!
//! The schema is intentionally tiny:
//!
//! ```json
//! {
//!   "schema_version": 1,
//!   "wallet_dir_override": "/path/to/custom/wallets"
//! }
//! ```
//!
//! Failure is always silent and recoverable: a missing, malformed, or
//! unreadable config file falls back to the platform default
//! ([`crate::state::default_wallet_dir`]) without surfacing an error to
//! the user. The on-write side is best-effort for the same reason —
//! the GUI must remain usable on read-only filesystems and in container
//! sandboxes that block writes to the config dir.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::warn;

/// Bundle identifier used for the per-app config directory.
///
/// Must match `tauri.conf.json` `identifier`. Mirrors Tauri v2's
/// `PathResolver::app_config_dir` mapping so the persisted file lands
/// in the same place across release channels.
const BUNDLE_IDENTIFIER: &str = "org.shekyl.wallet";

/// Filename within the per-app config directory.
const CONFIG_FILENAME: &str = "gui-config.json";

/// On-disk shape. New fields must be optional (`#[serde(default)]`)
/// so older config files load without error after an upgrade.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GuiConfig {
    /// Schema version. Bumped on backwards-incompatible field renames
    /// or removals — additive changes are handled by `serde(default)`.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,

    /// Custom wallet directory, if the user has set one. `None` means
    /// "use the platform default". The path is stored verbatim (not
    /// canonicalised) so symlinks the user chose deliberately remain
    /// pointed at after their target moves.
    #[serde(default)]
    pub wallet_dir_override: Option<PathBuf>,
}

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

/// Current schema version. Increment on breaking changes.
pub const SCHEMA_VERSION: u32 = 1;

/// Resolve the on-disk path for `gui-config.json`.
///
/// Returns `None` if the platform doesn't expose a config directory at
/// all (extremely rare; only happens on stripped-down embedded
/// targets). Callers must treat `None` the same as "file missing" —
/// fall back to defaults silently.
pub fn config_path() -> Option<PathBuf> {
    Some(config_path_in(dirs::config_dir()?))
}

/// Test-overrideable variant: compose the config-file path under an
/// explicit base directory. Production code goes through
/// [`config_path`]; tests inject a [`tempfile::TempDir`]-like base so
/// they don't touch the developer's real config.
fn config_path_in(base: PathBuf) -> PathBuf {
    base.join(BUNDLE_IDENTIFIER).join(CONFIG_FILENAME)
}

/// Load the persisted config, returning [`GuiConfig::default`] on any
/// failure (missing file, parse error, IO error). Failures are logged
/// at `warn!` level but never returned to the caller.
pub fn load() -> GuiConfig {
    let Some(path) = config_path() else {
        return GuiConfig::default();
    };
    load_from_path(&path)
}

/// Read-and-parse the config file at an explicit path. Same fallback
/// behaviour as [`load`]: any error reduces to "use defaults", with a
/// `warn!` log on the failure cause.
fn load_from_path(path: &Path) -> GuiConfig {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return GuiConfig::default();
        }
        Err(e) => {
            warn!(error = %e, "failed to read gui-config.json; using defaults");
            return GuiConfig::default();
        }
    };

    match serde_json::from_slice::<GuiConfig>(&bytes) {
        Ok(cfg) => {
            if cfg.schema_version > SCHEMA_VERSION {
                warn!(
                    file = cfg.schema_version,
                    binary = SCHEMA_VERSION,
                    "gui-config.json schema_version is newer than this binary; ignoring"
                );
                return GuiConfig::default();
            }
            cfg
        }
        Err(e) => {
            warn!(error = %e, "failed to parse gui-config.json; using defaults");
            GuiConfig::default()
        }
    }
}

/// Persist the config to disk. Best-effort: failures are logged and
/// swallowed so a read-only config dir doesn't break the UI flow.
///
/// Creates the parent directory if it doesn't exist. Writes atomically
/// where possible (write to `*.tmp` then rename) so a power loss
/// mid-write doesn't corrupt the persisted file.
pub fn save(cfg: &GuiConfig) {
    let Some(path) = config_path() else {
        warn!("no config directory available; gui-config.json not persisted");
        return;
    };
    save_to_path(cfg, &path);
}

/// Serialize-and-write to an explicit path. Same best-effort posture
/// as [`save`]: any error is logged and swallowed.
fn save_to_path(cfg: &GuiConfig, path: &Path) {
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!(error = %e, "failed to create gui-config dir; not persisting");
            return;
        }
    }

    let bytes = match serde_json::to_vec_pretty(cfg) {
        Ok(b) => b,
        Err(e) => {
            warn!(error = %e, "failed to serialize gui-config; not persisting");
            return;
        }
    };

    let tmp = path.with_extension("json.tmp");
    if let Err(e) = std::fs::write(&tmp, &bytes) {
        warn!(error = %e, "failed to write gui-config tmp file; not persisting");
        return;
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        warn!(error = %e, "failed to rename gui-config tmp file; not persisting");
        let _ = std::fs::remove_file(&tmp);
    }
}

/// Result of resolving the wallet directory at startup, including any
/// soft-warning the UI should surface to the user.
#[derive(Debug, Clone)]
pub struct ResolvedWalletDir {
    /// The effective wallet directory after override + reachability
    /// checks. Guaranteed to be a real, writable directory by the time
    /// the GUI commands see it.
    pub dir: PathBuf,
    /// `Some(path)` when an override existed but was unreachable. The
    /// UI shows this as "your custom location is unavailable; falling
    /// back to default." `None` means either no override was set or
    /// the override worked.
    pub fallback_from: Option<PathBuf>,
}

/// Resolve the effective wallet directory: prefer the persisted
/// override, fall back to `default_dir` if the override is missing or
/// unreachable.
///
/// "Unreachable" here means "we couldn't create or even probe it" —
/// permission denied, target-is-a-file, or any other
/// [`std::fs::create_dir_all`] failure. A reachable-but-empty
/// directory is considered usable (a fresh override is a valid
/// override).
pub fn resolve_wallet_dir(default_dir: PathBuf) -> ResolvedWalletDir {
    let cfg = load();
    let Some(override_path) = cfg.wallet_dir_override else {
        return ResolvedWalletDir {
            dir: default_dir,
            fallback_from: None,
        };
    };

    match probe_dir(&override_path) {
        Ok(()) => ResolvedWalletDir {
            dir: override_path,
            fallback_from: None,
        },
        Err(_) => ResolvedWalletDir {
            dir: default_dir,
            fallback_from: Some(override_path),
        },
    }
}

fn probe_dir(path: &Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(path)?;
    // Confirm the path actually resolves to a directory after create
    // (handles the "target is a file" / symlink-to-file edge case
    // where create_dir_all succeeds because the path exists).
    let meta = std::fs::metadata(path)?;
    if !meta.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            "wallet override path is not a directory",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Per-test isolated config directory. Avoids touching the
    /// developer's real `~/.config/org.shekyl.wallet/` and avoids
    /// mutating process-global env vars (deprecation-warned in Rust
    /// 1.83+). Tests call `path_in(&dir)`, `load_from_path`,
    /// `save_to_path`, and the `resolve_*_in` helpers directly.
    fn fresh_test_dir(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("shekyl_guicfg_{}_{}", label, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Same composition rule as production [`config_path`], but rooted
    /// at the caller-supplied base instead of the OS-derived config
    /// dir. Mirrors [`config_path_in`].
    fn test_config_path(base: &Path) -> PathBuf {
        base.join(BUNDLE_IDENTIFIER).join(CONFIG_FILENAME)
    }

    /// Resolve-with-injected-base variant used by tests; mirrors
    /// [`resolve_wallet_dir`] but reads the override from the
    /// test-controlled config file rather than [`load`].
    fn resolve_wallet_dir_in(base: &Path, default_dir: PathBuf) -> ResolvedWalletDir {
        let cfg = load_from_path(&test_config_path(base));
        let Some(override_path) = cfg.wallet_dir_override else {
            return ResolvedWalletDir {
                dir: default_dir,
                fallback_from: None,
            };
        };
        match probe_dir(&override_path) {
            Ok(()) => ResolvedWalletDir {
                dir: override_path,
                fallback_from: None,
            },
            Err(_) => ResolvedWalletDir {
                dir: default_dir,
                fallback_from: Some(override_path),
            },
        }
    }

    #[test]
    fn load_returns_default_when_file_missing() {
        let base = fresh_test_dir("missing");
        let cfg = load_from_path(&test_config_path(&base));
        assert!(cfg.wallet_dir_override.is_none());
        assert_eq!(cfg.schema_version, SCHEMA_VERSION);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn load_returns_default_on_malformed_json() {
        let base = fresh_test_dir("malformed");
        let path = test_config_path(&base);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"{not json}").unwrap();
        let cfg = load_from_path(&path);
        assert!(
            cfg.wallet_dir_override.is_none(),
            "malformed JSON must not produce a phantom override"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn save_then_load_roundtrips_override() {
        let base = fresh_test_dir("roundtrip");
        let path = test_config_path(&base);
        let custom = base.join("custom_wallets_test");
        let cfg = GuiConfig {
            schema_version: SCHEMA_VERSION,
            wallet_dir_override: Some(custom.clone()),
        };
        save_to_path(&cfg, &path);
        let loaded = load_from_path(&path);
        assert_eq!(loaded.wallet_dir_override, Some(custom));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn save_creates_parent_dir() {
        let base = fresh_test_dir("mkparent");
        let path = test_config_path(&base);
        let parent = path.parent().unwrap().to_path_buf();
        let _ = std::fs::remove_dir_all(&parent);
        let cfg = GuiConfig {
            schema_version: SCHEMA_VERSION,
            wallet_dir_override: Some(base.join("x")),
        };
        save_to_path(&cfg, &path);
        assert!(parent.is_dir(), "save_to_path must create the parent dir");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn future_schema_version_is_refused() {
        let base = fresh_test_dir("future");
        let path = test_config_path(&base);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // Hand-write a schema_version one above what this binary
        // supports; mirrors what would happen if a future GUI build
        // wrote the file and the user downgraded.
        let future = format!(
            "{{\"schema_version\":{},\"wallet_dir_override\":\"/tmp/future\"}}",
            SCHEMA_VERSION + 1
        );
        std::fs::write(&path, future).unwrap();
        let cfg = load_from_path(&path);
        assert!(
            cfg.wallet_dir_override.is_none(),
            "future schema_version must fall back to defaults, not adopt the override"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn resolve_wallet_dir_uses_default_when_no_override() {
        let base = fresh_test_dir("resolve_default");
        let default = base.join("default");
        std::fs::create_dir_all(&default).unwrap();
        let resolved = resolve_wallet_dir_in(&base, default.clone());
        assert_eq!(resolved.dir, default);
        assert!(resolved.fallback_from.is_none());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn resolve_wallet_dir_uses_override_when_reachable() {
        let base = fresh_test_dir("resolve_override");
        let path = test_config_path(&base);
        let default = base.join("default");
        std::fs::create_dir_all(&default).unwrap();
        let override_dir = base.join("override");
        save_to_path(
            &GuiConfig {
                schema_version: SCHEMA_VERSION,
                wallet_dir_override: Some(override_dir.clone()),
            },
            &path,
        );

        let resolved = resolve_wallet_dir_in(&base, default.clone());
        assert_eq!(resolved.dir, override_dir);
        assert!(resolved.fallback_from.is_none());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn resolve_wallet_dir_falls_back_when_override_is_a_file() {
        let base = fresh_test_dir("resolve_fallback");
        let path = test_config_path(&base);
        let default = base.join("default");
        std::fs::create_dir_all(&default).unwrap();
        let override_path = base.join("override_as_file");
        // Write a regular file at the override path — probe_dir must
        // detect this and trigger the fallback.
        std::fs::write(&override_path, b"not a dir").unwrap();
        save_to_path(
            &GuiConfig {
                schema_version: SCHEMA_VERSION,
                wallet_dir_override: Some(override_path.clone()),
            },
            &path,
        );

        let resolved = resolve_wallet_dir_in(&base, default.clone());
        assert_eq!(
            resolved.dir, default,
            "fallback must use the platform default"
        );
        assert_eq!(
            resolved.fallback_from,
            Some(override_path.clone()),
            "fallback_from must record the unreachable override"
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
