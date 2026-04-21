// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without modification, are
// permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice, this list of
//    conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice, this list
//    of conditions and the following disclaimer in the documentation and/or other
//    materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors may be
//    used to endorse or promote products derived from this software without specific
//    prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY
// EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL
// THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
// PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF
// THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

//! Wallet filename and directory utilities.
//!
//! The GUI accepts human-friendly wallet names ("My Wallet") and saves them
//! as underscored, filesystem-friendly files ("My_Wallet.keys"). The
//! `wallet2_ffi` layer no longer joins directory + filename (see
//! `shekyl-core` CHANGELOG: "wallet2_ffi no longer carries wallet-directory
//! state"); this module owns that job so the separator is always correct
//! for the host platform via `PathBuf::join`.

use std::path::{Path, PathBuf};

/// Normalize a user-supplied wallet name for disk storage.
///
/// Trims surrounding whitespace, collapses runs of internal Unicode
/// whitespace into single spaces, and replaces those spaces with
/// underscores. Idempotent: `sanitize(sanitize(x)) == sanitize(x)`.
///
/// The goal is "My Wallet" → "My_Wallet", not to be a general
/// filesystem-safe sanitizer. Characters that are unsafe on Windows
/// (`<>:"/\|?*`) are rejected upstream by [`crate::validate::validate_wallet_name`];
/// this function assumes its input already passed that check and focuses
/// only on whitespace handling.
pub fn sanitize(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut out = String::with_capacity(trimmed.len());
    let mut in_ws = false;
    for c in trimmed.chars() {
        if c.is_whitespace() {
            if !in_ws {
                out.push('_');
                in_ws = true;
            }
        } else {
            out.push(c);
            in_ws = false;
        }
    }
    out
}

/// Join a wallet directory and a name into a full path using the host
/// separator. The caller is responsible for pre-sanitizing the name
/// via [`sanitize`].
pub fn build_wallet_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(name)
}

/// Ensure the wallet directory exists, creating it (and any missing
/// parents) if necessary. Equivalent to `mkdir -p` on POSIX; on Windows
/// [`std::fs::create_dir_all`] handles nested paths the same way.
///
/// Returns a user-safe error message that does not echo the input path
/// back, preserving the no-secret-leakage posture asserted in
/// [`crate::validate`] tests.
pub fn ensure_dir_exists(dir: &Path) -> Result<(), String> {
    match std::fs::create_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(e) => Err(match e.kind() {
            std::io::ErrorKind::PermissionDenied => {
                "Permission denied creating wallet directory".into()
            }
            std::io::ErrorKind::AlreadyExists => {
                // create_dir_all only returns AlreadyExists when the
                // target path exists but is not a directory — a regular
                // file, a symlink to a file, etc.
                "Wallet directory path exists but is not a directory".into()
            }
            _ => "Could not create wallet directory".into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── sanitize ───────────────────────────────────────────────────────────

    #[test]
    fn sanitize_leaves_simple_name_unchanged() {
        assert_eq!(sanitize("wallet"), "wallet");
        assert_eq!(sanitize("MyWallet"), "MyWallet");
    }

    #[test]
    fn sanitize_replaces_single_space() {
        assert_eq!(sanitize("My Wallet"), "My_Wallet");
    }

    #[test]
    fn sanitize_trims_surrounding_whitespace() {
        assert_eq!(sanitize("  My Wallet  "), "My_Wallet");
        assert_eq!(sanitize("\tMy Wallet\n"), "My_Wallet");
    }

    #[test]
    fn sanitize_collapses_multiple_spaces() {
        assert_eq!(sanitize("My   Wallet"), "My_Wallet");
        assert_eq!(sanitize("A  B  C"), "A_B_C");
    }

    #[test]
    fn sanitize_collapses_mixed_whitespace() {
        assert_eq!(sanitize("A\tB"), "A_B");
        assert_eq!(sanitize("A \t  B"), "A_B");
        assert_eq!(sanitize("A\u{00A0}B"), "A_B"); // non-breaking space
    }

    #[test]
    fn sanitize_empty_and_all_whitespace() {
        assert_eq!(sanitize(""), "");
        assert_eq!(sanitize("   "), "");
        assert_eq!(sanitize("\t\n "), "");
    }

    #[test]
    fn sanitize_is_idempotent() {
        let cases = ["My Wallet", "A  B  C", "  wallet  ", "wallet", ""];
        for c in cases {
            let once = sanitize(c);
            let twice = sanitize(&once);
            assert_eq!(once, twice, "not idempotent for input {c:?}");
        }
    }

    #[test]
    fn sanitize_preserves_unicode_letters() {
        assert_eq!(sanitize("café wallet"), "café_wallet");
    }

    // ── build_wallet_path ──────────────────────────────────────────────────

    #[test]
    fn build_wallet_path_joins_with_host_separator() {
        let dir = PathBuf::from("/tmp/shekyl/wallets");
        let p = build_wallet_path(&dir, "My_Wallet");
        // On POSIX this is "/tmp/shekyl/wallets/My_Wallet";
        // on Windows "\" is the separator. Either way the path ends with
        // the name as a single component.
        assert_eq!(p.file_name().unwrap(), "My_Wallet");
        assert_eq!(p.parent().unwrap(), dir);
    }

    // ── ensure_dir_exists ──────────────────────────────────────────────────

    #[test]
    fn ensure_dir_exists_creates_fresh_dir() {
        let base = std::env::temp_dir().join(format!("shekyl_test_fresh_{}", std::process::id()));
        let nested = base.join("a").join("b").join("c");
        assert!(!nested.exists());
        ensure_dir_exists(&nested).unwrap();
        assert!(nested.is_dir());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn ensure_dir_exists_is_idempotent() {
        let base = std::env::temp_dir().join(format!("shekyl_test_idem_{}", std::process::id()));
        ensure_dir_exists(&base).unwrap();
        ensure_dir_exists(&base).unwrap();
        ensure_dir_exists(&base).unwrap();
        assert!(base.is_dir());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn ensure_dir_exists_fails_when_target_is_file() {
        let base = std::env::temp_dir().join(format!("shekyl_test_filecol_{}", std::process::id()));
        let _ = std::fs::remove_file(&base);
        std::fs::write(&base, b"not a directory").unwrap();

        let err = ensure_dir_exists(&base).unwrap_err();
        assert!(!err.is_empty(), "error should be non-empty");
        // No path leakage — temp_dir() returns a user-derived path.
        assert!(
            !err.contains(&base.to_string_lossy().to_string()),
            "error leaked the input path"
        );
        let _ = std::fs::remove_file(&base);
    }
}
