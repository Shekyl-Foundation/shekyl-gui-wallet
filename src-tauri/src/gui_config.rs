// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Persistent GUI preferences stored as `gui-config.json` in the GUI's
//! settings directory in the per-account root
//! ([`crate::account_root::settings_dir`]).
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
//! unreadable config file falls back to the default
//! ([`crate::account_root::default_wallet_dir`]) without surfacing an error to
//! the user. The on-write side is best-effort for the same reason —
//! the GUI must remain usable on read-only filesystems and in container
//! sandboxes that block writes to the config dir.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::warn;

/// Filename within the per-app config directory.
const CONFIG_FILENAME: &str = "gui-config.json";

/// On-disk shape. New fields must be optional (`#[serde(default)]`)
/// so older config files load without error after an upgrade.
///
/// `Default` is implemented by hand rather than derived: the derived
/// version would zero-initialise `schema_version` (ignoring the
/// `#[serde(default = ...)]` attribute, which only applies to
/// deserialization), so a missing config file would load as
/// `schema_version: 0` instead of the current [`SCHEMA_VERSION`].
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// The network the person chose, by its name (`mainnet`, `testnet`,
    /// `stagenet`). The wallet's node starts on it at launch, so no node
    /// runs on another network first. `None` on a fresh install: the first
    /// start is mainnet (Rick, 2026-11-13, 2026-11-17).
    #[serde(default)]
    pub network: Option<String>,
}

impl GuiConfig {
    /// The network the wallet's node starts on: the saved one, or mainnet
    /// when none is saved. A name this build does not know is mainnet too,
    /// said in the log.
    pub fn startup_network(&self) -> crate::state::NetworkType {
        let Some(name) = &self.network else {
            return crate::state::NetworkType::default();
        };
        crate::state::NetworkType::from_name(name).unwrap_or_else(|| {
            warn!(network = %name, "gui-config.json names a network this build does not know; starting on mainnet");
            crate::state::NetworkType::default()
        })
    }
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            wallet_dir_override: None,
            network: None,
        }
    }
}

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

/// Current schema version. Increment on breaking changes.
pub const SCHEMA_VERSION: u32 = 1;

/// The on-disk path for `gui-config.json`: in the GUI's settings
/// directory ([`crate::account_root::settings_dir`]). `None` when the
/// account has no home: nothing is read and nothing is kept.
pub fn config_path() -> Option<PathBuf> {
    crate::account_root::settings_dir().map(|dir| dir.join(CONFIG_FILENAME))
}

/// Where the GUI's settings are kept: one file, named once, when the app
/// starts. Everything that reads or writes the settings goes through the
/// store the app's state holds, so a test's state, which names a file of
/// its own or none, never reads or writes the settings of the account the
/// tests run under.
#[derive(Debug, Clone)]
pub struct Store(Option<PathBuf>);

impl Store {
    /// This account's settings file ([`config_path`]).
    pub fn of_this_account() -> Self {
        Self(config_path())
    }

    /// No file: nothing is read and nothing is kept. A test's store.
    #[cfg(test)]
    pub(crate) const fn none() -> Self {
        Self(None)
    }

    /// A test's own file.
    #[cfg(test)]
    pub(crate) fn at(path: PathBuf) -> Self {
        Self(Some(path))
    }

    /// Load the persisted config, returning [`GuiConfig::default`] on any
    /// failure (missing file, parse error, IO error). Failures are logged
    /// at `warn!` level but never returned to the caller.
    pub fn load(&self) -> GuiConfig {
        self.0
            .as_deref()
            .map_or_else(GuiConfig::default, load_from_path)
    }

    /// Persist the config to disk. Best-effort: failures are logged and
    /// swallowed so a read-only config dir doesn't break the UI flow.
    ///
    /// Creates the parent directory if it doesn't exist. Writes atomically
    /// where possible (write to `*.tmp` then rename) so a power loss
    /// mid-write doesn't corrupt the persisted file.
    pub fn save(&self, cfg: &GuiConfig) {
        match &self.0 {
            Some(path) => save_to_path(cfg, path),
            None => warn!("no config directory available; gui-config.json not persisted"),
        }
    }

    /// Keep `network` as the network the wallet's node starts on next
    /// time, leaving the rest of the file as it is.
    pub fn save_network(&self, network: crate::state::NetworkType) {
        let mut cfg = self.load();
        cfg.network = Some(network.as_str().to_owned());
        self.save(&cfg);
    }
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

/// Serialize-and-write to an explicit path. Same best-effort posture
/// as [`save`]: any error is logged and swallowed.
fn save_to_path(cfg: &GuiConfig, path: &Path) {
    if let Some(parent) = path.parent() {
        if let Err(e) = shekyl_os_account::make_account_dir(parent) {
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
    /// checks: a reachable override, else the default, which a command
    /// makes when it first needs it. `None` when neither exists: the
    /// account has no home and the override is unset or unreachable.
    pub dir: Option<PathBuf>,
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
pub fn resolve_wallet_dir(cfg: GuiConfig, default_dir: Option<PathBuf>) -> ResolvedWalletDir {
    let Some(override_path) = cfg.wallet_dir_override else {
        return ResolvedWalletDir {
            dir: default_dir,
            fallback_from: None,
        };
    };

    match probe_dir(&override_path) {
        Ok(()) => ResolvedWalletDir {
            dir: Some(override_path),
            fallback_from: None,
        },
        Err(_) => ResolvedWalletDir {
            dir: default_dir,
            fallback_from: Some(override_path),
        },
    }
}

fn probe_dir(path: &Path) -> Result<(), std::io::Error> {
    shekyl_os_account::make_account_dir(path)?;
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

    /// A test's own settings directory under `base`, with the file named
    /// as production names it ([`config_path`]), so a test never touches
    /// the settings of the account it runs under.
    fn test_config_path(base: &Path) -> PathBuf {
        base.join(crate::account_root::SETTINGS_DIR_NAME)
            .join(CONFIG_FILENAME)
    }

    /// [`resolve_wallet_dir`] over a test's own config file.
    fn resolve_wallet_dir_in(base: &Path, default_dir: PathBuf) -> ResolvedWalletDir {
        resolve_wallet_dir(load_from_path(&test_config_path(base)), Some(default_dir))
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
            network: None,
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
            network: None,
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
        assert_eq!(resolved.dir, Some(default));
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
                network: None,
            },
            &path,
        );

        let resolved = resolve_wallet_dir_in(&base, default.clone());
        assert_eq!(resolved.dir, Some(override_dir));
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
                network: None,
            },
            &path,
        );

        let resolved = resolve_wallet_dir_in(&base, default.clone());
        assert_eq!(
            resolved.dir,
            Some(default),
            "fallback must use the platform default"
        );
        assert_eq!(
            resolved.fallback_from,
            Some(override_path.clone()),
            "fallback_from must record the unreachable override"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// An account with no home has no default wallet directory (rule 83
    /// IG-13): with no override, or an unreachable one, there is none, and
    /// an unreachable override is still reported. Red edit: a `"."`
    /// fallback (the old default) puts wallets in the working directory.
    #[test]
    fn no_home_and_no_reachable_override_is_no_wallet_dir() {
        let base = fresh_test_dir("resolve_no_home");
        let path = test_config_path(&base);
        let resolved = resolve_wallet_dir(load_from_path(&path), None);
        assert_eq!(resolved.dir, None);
        assert_eq!(resolved.fallback_from, None);

        let override_path = base.join("override_as_file");
        std::fs::write(&override_path, b"not a dir").unwrap();
        save_to_path(
            &GuiConfig {
                schema_version: SCHEMA_VERSION,
                wallet_dir_override: Some(override_path.clone()),
                network: None,
            },
            &path,
        );
        let resolved = resolve_wallet_dir(load_from_path(&path), None);
        assert_eq!(resolved.dir, None);
        assert_eq!(resolved.fallback_from, Some(override_path));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The node starts on the saved network; a fresh install, and a name
    /// this build does not know, start on mainnet. The choice survives the
    /// wallet directory being set. Red edit: `startup_network` returns the
    /// default (every launch starts mainnet first).
    #[test]
    fn the_node_starts_on_the_saved_network() {
        use crate::state::NetworkType;
        let fresh = GuiConfig::default();
        assert_eq!(fresh.startup_network(), NetworkType::Mainnet);
        let saved = |name: &str| GuiConfig {
            network: Some(name.to_owned()),
            ..GuiConfig::default()
        };
        for net in NetworkType::ALL {
            assert_eq!(saved(net.as_str()).startup_network(), net);
        }
        assert_eq!(
            saved("a-net-from-later").startup_network(),
            NetworkType::Mainnet
        );
        // An older file without the key reads as a fresh one.
        let old: GuiConfig =
            serde_json::from_str(r#"{"schema_version": 1, "wallet_dir_override": null}"#).unwrap();
        assert_eq!(old.startup_network(), NetworkType::Mainnet);
    }
}
