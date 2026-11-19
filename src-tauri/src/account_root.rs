// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! The GUI's places in this account's per-account root, as shekyl-core
//! names it (core rule 83 IG-13 and IG-Q29; `shekyl_os_account`):
//! `~/.shekyl` on Linux, FreeBSD and macOS, `%LOCALAPPDATA%\Shekyl` on
//! Windows, read from the account interface, never `HOME`, `XDG_*` or
//! `%APPDATA%`. The wallet directory and the GUI's settings sit in it.
//!
//! The app makes the root once, at its start ([`AccountRoot::ensure`]), as
//! the node makes it, and every place in it is read off what that start
//! left: a root made with its protections, or the reason nothing may be
//! made in it. A root that could not be made with its protections is never
//! made another way: on Windows a plain `create_dir_all` of a directory
//! inside it would build the root with inherited permissions and no
//! no-read-up label, and core leaves an existing root as it finds it, so
//! the unprotected root would stay.

use std::path::{Path, PathBuf};

/// The GUI's settings directory's name in the per-account root: the
/// wallet's settings (`gui-config.json`), its node's settings
/// (`daemon.json`) and the node's error output (`shekyld.stderr`).
pub const SETTINGS_DIR_NAME: &str = "shekyl-gui-wallet";

/// What a command that needs the wallet directory says when the account
/// has no home.
pub const NO_WALLET_DIR: &str =
    "This account has no home folder, so there is no default wallet folder. Choose a folder for your wallets.";

/// What the wallet's node says when the account has no home: it has
/// nowhere to keep its settings, so it is not started and nothing is
/// saved.
pub const NO_SETTINGS_DIR: &str =
    "This account has no home folder, so the wallet's node has nowhere to keep its settings and is not started.";

/// This account's per-account root as the app's start left it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountRoot {
    /// Made, or found, with its protections: the wallet directory and the
    /// settings are made inside it.
    Ready(PathBuf),
    /// The account has no home: there is no root, no default wallet
    /// directory and no settings.
    NoHome,
    /// The root could not be made with its protections. Nothing is made in
    /// it until a later start makes it.
    NotMade {
        /// Where the root would be.
        root: PathBuf,
        /// What failed, in the operating system's words.
        error: String,
    },
}

impl AccountRoot {
    /// Make this account's per-account root where it does not exist, as the
    /// node makes it (owner-only with the no-read-up label on Windows;
    /// `shekyl_os_account::ensure_per_account_root`). Called once, at the
    /// app's start, before anything is made in the root: every directory
    /// made in it afterwards, under whatever spelling of its path, finds it
    /// made with its protections, whichever of the wallet and its node ran
    /// first.
    pub fn ensure() -> Self {
        Self::made_by(
            shekyl_os_account::per_account_root(),
            shekyl_os_account::ensure_per_account_root,
        )
    }

    /// The root at `root`, made by `make`. Split from [`Self::ensure`] so a
    /// test names the root and the failure.
    fn made_by(root: Option<PathBuf>, make: impl FnOnce(&Path) -> std::io::Result<()>) -> Self {
        let Some(root) = root else {
            return Self::NoHome;
        };
        match make(&root) {
            Ok(()) => Self::Ready(root),
            Err(e) => {
                tracing::warn!(root = %root.display(), error = %e, "the per-account root could not be made");
                Self::NotMade {
                    root,
                    error: e.to_string(),
                }
            }
        }
    }

    /// The wallet directory when the person names none: `wallets` in the
    /// root (`shekyl_os_account::wallet_dir_in`). Without a root there is
    /// no default, the person chooses a directory, and the words say why.
    pub fn default_wallet_dir(&self) -> Result<PathBuf, String> {
        match self {
            Self::Ready(root) => Ok(shekyl_os_account::wallet_dir_in(root)),
            Self::NoHome => Err(NO_WALLET_DIR.to_owned()),
            Self::NotMade { root, error } => Err(format!(
                "The wallet could not make its folder {} ({error}), so there is no default wallet folder. Choose a folder for your wallets.",
                root.display()
            )),
        }
    }

    /// The GUI's settings directory, [`SETTINGS_DIR_NAME`] in the root.
    /// Without a root nothing is kept, the wallet's node is not started,
    /// and the words say why.
    pub fn settings_dir(&self) -> Result<PathBuf, String> {
        match self {
            Self::Ready(root) => Ok(root.join(SETTINGS_DIR_NAME)),
            Self::NoHome => Err(NO_SETTINGS_DIR.to_owned()),
            Self::NotMade { root, error } => Err(format!(
                "The wallet could not make its folder {} ({error}), so the wallet's node has nowhere to keep its settings and is not started.",
                root.display()
            )),
        }
    }

    /// A root that could not be made, at `root`, for a test of what the app
    /// then does.
    #[cfg(test)]
    pub(crate) fn not_made_at(root: PathBuf) -> Self {
        Self::made_by(Some(root), |_| {
            Err(std::io::Error::other("the owner could not be had"))
        })
    }
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
        // The account's root, named and not made: the test makes nothing
        // in the home of the account it runs under.
        let found = AccountRoot::made_by(shekyl_os_account::per_account_root(), |_| Ok(()));
        assert_eq!(found, AccountRoot::Ready(root.clone()));
        assert_eq!(found.default_wallet_dir(), Ok(root.join("wallets")));
        assert_eq!(found.settings_dir(), Ok(root.join("shekyl-gui-wallet")));
    }

    /// A root that could not be made with its protections gives no place
    /// in it: the wallet directory and the settings refuse, naming the
    /// root and what failed, so nothing later makes the root without its
    /// protections. Red edit: making `made_by` return `Ready` after a
    /// failed make (the old log-and-carry-on) fails this test.
    #[test]
    fn a_root_not_made_gives_no_place_in_it_and_says_why() {
        let root = std::env::temp_dir().join("shekyl-gui-root-not-made");
        let refused = AccountRoot::not_made_at(root.clone());
        assert_eq!(
            refused,
            AccountRoot::NotMade {
                root: root.clone(),
                error: "the owner could not be had".to_owned(),
            }
        );
        for words in [
            refused.default_wallet_dir().unwrap_err(),
            refused.settings_dir().unwrap_err(),
        ] {
            assert!(words.contains(&root.display().to_string()), "{words}");
            assert!(words.contains("the owner could not be had"), "{words}");
        }
    }

    /// An account with no home has no root and no place in one; the words
    /// say so.
    #[test]
    fn no_home_gives_no_place_and_says_so() {
        let none = AccountRoot::made_by(None, |_| panic!("nothing is made without a home"));
        assert_eq!(none, AccountRoot::NoHome);
        assert_eq!(none.default_wallet_dir(), Err(NO_WALLET_DIR.to_owned()));
        assert_eq!(none.settings_dir(), Err(NO_SETTINGS_DIR.to_owned()));
    }
}
