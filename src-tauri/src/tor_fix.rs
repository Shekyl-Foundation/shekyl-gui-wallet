// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! A Tor fix a person took, and the node's launch line for it
//! (`docs/design/GUI_SYNC_LANE.md` GS-3, GS-F7).
//!
//! The node's Tor read offers fixes for an install cause (shekyl-core
//! `shekyl-tor-status`, `Fix`). Taking one is the person's act: it is
//! stored in the wallet's settings and passed on the node's launch line,
//! and the node is started again. The daemon's default configuration file
//! is never written: that file is shared with a node the person runs by
//! hand (Rick, 2026-11-17).
//!
//! The wallet does not judge a fix's value (a Tor's path, a SOCKS
//! address): the daemon does, at its start, and its refusal is shown in
//! its own words.
//!
//! A fix is the whole Tor part of the launch line, never arguments added
//! to another's. The daemon refuses `tor-file` unless `tor` is `managed`,
//! and `tor-socks` unless it is `operator` (shekyl-core `INPUT_GRAMMAR.md`
//! §5), so the bundled Tor is named only where a managed Tor runs.

use std::ffi::OsString;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The fix in force, as the wallet's settings keep it. None is the
/// default: the node's own defaults, with the bundled Tor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "fix", rename_all = "kebab-case")]
pub enum TorFix {
    /// `restore-managed`, by naming a Tor: Tor stays managed, and this file
    /// is the Tor the node checks and runs.
    RestoreManaged {
        /// The Tor's path (`--tor-file`).
        tor_file: String,
    },
    /// `operator`: a Tor the person runs.
    Operator {
        /// That Tor's SOCKS endpoint (`--tor-socks`), which the daemon
        /// requires with it.
        tor_socks: String,
    },
    /// `clearnet-on`: clearnet on, Tor still managed.
    ClearnetOn,
    /// `clearnet-only`: Tor off, and the node sends over the open network.
    ClearnetOnly,
}

impl TorFix {
    /// The keyword of core's `Fix` this is.
    pub(crate) const fn keyword(&self) -> &'static str {
        match self {
            Self::RestoreManaged { .. } => "restore-managed",
            Self::Operator { .. } => "operator",
            Self::ClearnetOn => "clearnet-on",
            Self::ClearnetOnly => "clearnet-only",
        }
    }
}

/// The Tor part of the node's launch line: `fix`, or with none the
/// bundled Tor where the build has one.
pub(crate) fn tor_args(fix: Option<&TorFix>, bundled: Option<&Path>) -> Vec<OsString> {
    let managed_file = |file: Option<OsString>| -> Vec<OsString> {
        file.map_or_else(Vec::new, |file| vec!["--tor-file".into(), file])
    };
    let bundled = bundled.map(|tor| tor.as_os_str().to_owned());
    match fix {
        None => managed_file(bundled),
        Some(TorFix::RestoreManaged { tor_file }) => managed_file(Some(tor_file.into())),
        Some(TorFix::ClearnetOn) => {
            let mut args = managed_file(bundled);
            args.extend(["--clearnet".into(), "on".into()]);
            args
        }
        Some(TorFix::Operator { tor_socks }) => vec![
            "--tor".into(),
            "operator".into(),
            "--tor-socks".into(),
            tor_socks.into(),
        ],
        Some(TorFix::ClearnetOnly) => vec![
            "--tor".into(),
            "off".into(),
            "--clearnet".into(),
            "on".into(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(fix: Option<&TorFix>) -> Vec<String> {
        tor_args(fix, Some(Path::new("/res/tor/tor")))
            .into_iter()
            .map(|a| a.into_string().unwrap())
            .collect()
    }

    /// Each fix is the whole Tor part of the launch line, with the bundled
    /// Tor in the build. The daemon refuses `tor-file` beside a `tor` that
    /// is not `managed`, so a fix that turns Tor off or names an operator's
    /// carries no `--tor-file`. Red edit: append a fix's arguments to the
    /// bundled Tor's (the node then refuses its start for `clearnet-only`
    /// and `operator` in every installed build, and in no test whose build
    /// has no Tor).
    #[test]
    fn each_fix_is_the_whole_tor_part_of_the_launch_line() {
        assert_eq!(line(None), ["--tor-file", "/res/tor/tor"]);
        assert_eq!(
            line(Some(&TorFix::ClearnetOn)),
            ["--tor-file", "/res/tor/tor", "--clearnet", "on"]
        );
        assert_eq!(
            line(Some(&TorFix::ClearnetOnly)),
            ["--tor", "off", "--clearnet", "on"]
        );
        assert_eq!(
            line(Some(&TorFix::Operator {
                tor_socks: "127.0.0.1:9050".to_owned()
            })),
            ["--tor", "operator", "--tor-socks", "127.0.0.1:9050"]
        );
        assert_eq!(
            line(Some(&TorFix::RestoreManaged {
                tor_file: "/opt/tor/tor".to_owned()
            })),
            ["--tor-file", "/opt/tor/tor"]
        );
        for fix in [
            TorFix::ClearnetOnly,
            TorFix::Operator {
                tor_socks: "127.0.0.1:9050".to_owned(),
            },
        ] {
            assert!(
                !line(Some(&fix)).contains(&"--tor-file".to_owned()),
                "{fix:?}"
            );
        }
        // A build with no Tor names none, and a fix is the same.
        assert!(tor_args(None, None).is_empty());
        assert_eq!(
            tor_args(Some(&TorFix::ClearnetOn), None),
            [OsString::from("--clearnet"), OsString::from("on")]
        );
    }

    /// The settings file keeps a fix by core's keyword, and a file written
    /// before fixes existed reads as none.
    #[test]
    fn a_fix_is_kept_by_core_s_keyword() {
        for fix in [
            TorFix::RestoreManaged {
                tor_file: "/t".to_owned(),
            },
            TorFix::Operator {
                tor_socks: "127.0.0.1:9050".to_owned(),
            },
            TorFix::ClearnetOn,
            TorFix::ClearnetOnly,
        ] {
            let json = serde_json::to_value(&fix).unwrap();
            assert_eq!(json["fix"], fix.keyword());
            assert_eq!(serde_json::from_value::<TorFix>(json).unwrap(), fix);
            let known = shekyl_tor_status::Fix::known();
            assert!(
                known.iter().any(|k| k.keyword() == fix.keyword()),
                "core has no fix `{}`",
                fix.keyword()
            );
        }
        assert_eq!(
            shekyl_tor_status::Fix::known().len(),
            4,
            "core's fixes changed: the wallet's settings keep each by name"
        );
    }
}
