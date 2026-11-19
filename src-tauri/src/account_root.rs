// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! The GUI's places in this account's per-account root, as shekyl-core
//! names it (core rule 83 IG-13 and IG-Q29; `shekyl_os_account`):
//! `~/.shekyl` on Linux, FreeBSD and macOS, `%LOCALAPPDATA%\Shekyl` on
//! Windows, read from the account interface, never `HOME`, `XDG_*` or
//! `%APPDATA%`. The wallet directory and the GUI's settings sit in it, and
//! every directory the GUI makes is made through
//! `shekyl_os_account::make_account_dir`, so the root is the one the node
//! makes.

use std::path::PathBuf;

/// The GUI's settings directory's name in the per-account root: the
/// wallet's settings (`gui-config.json`), its node's settings
/// (`daemon.json`) and the node's error output (`shekyld.stderr`).
pub const SETTINGS_DIR_NAME: &str = "shekyl-gui-wallet";

/// The wallet directory when the person names none: `wallets` in the
/// per-account root (`shekyl_os_account::wallet_dir_in`). `None` when the
/// account has no home: there is then no default, and the person chooses a
/// directory.
pub fn default_wallet_dir() -> Option<PathBuf> {
    shekyl_os_account::per_account_root().map(|root| shekyl_os_account::wallet_dir_in(&root))
}

/// The GUI's settings directory, [`SETTINGS_DIR_NAME`] in the per-account
/// root. `None` when the account has no home: nothing is kept.
pub fn settings_dir() -> Option<PathBuf> {
    shekyl_os_account::per_account_root().map(|root| root.join(SETTINGS_DIR_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wallet directory and the settings are in core's per-account
    /// root (rule 83 IG-13), pinned as the rule states it: never Roaming
    /// AppData, `Application Support` or `XDG_CONFIG_HOME`, which the
    /// regenesis upgrade does not clear. Red edit: the old per-platform
    /// defaults (`%APPDATA%\shekyl\wallets`, `~/Library/Application
    /// Support/shekyl/wallets`, Tauri's config folder) fail on Windows and
    /// macOS, and a `HOME` read fails under a moved `HOME`.
    #[test]
    fn the_wallet_and_settings_dirs_are_in_the_per_account_root() {
        let home = shekyl_os_account::account_home().expect("the test account has a home");
        let root = home.join(if cfg!(windows) { "Shekyl" } else { ".shekyl" });
        assert_eq!(default_wallet_dir(), Some(root.join("wallets")));
        assert_eq!(settings_dir(), Some(root.join("shekyl-gui-wallet")));
    }
}
