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

/// Filename policy: which characters are kept verbatim by [`sanitize`].
///
/// The accepted set is `[A-Za-z0-9_\-.]` plus any character that
/// [`char::is_alphabetic`] recognises as a Unicode letter (so non-ASCII
/// scripts like "café" round-trip without mangling). Every other
/// character — path separators, null bytes, Windows-reserved punctuation,
/// emoji, control bytes — is replaced with `_` and run-collapsed.
fn is_filename_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c.is_alphabetic() || matches!(c, '_' | '-' | '.')
}

/// Normalize an arbitrary user-supplied string into a filesystem-safe
/// wallet filename. **Single source of truth for filename policy.**
///
/// 1. Replace every character outside the accepted set
///    (see [`is_filename_char`]) with `_`.
/// 2. Collapse runs of `_` (whether originally present or produced by
///    rule 1) to a single `_`.
/// 3. Trim leading/trailing `_`, `.`, and whitespace. Leading dots are
///    stripped so the result is never a POSIX hidden file or a `.` /
///    `..` traversal token; trailing dots are stripped so Windows
///    doesn't quietly drop them at create-file time.
/// 4. Return the result (which may be empty if the input was entirely
///    unrepresentable, e.g. `////`). Callers couple this with
///    [`crate::validate::validate_wallet_name`] for the post-sanitize
///    empty / oversize check.
///
/// Idempotent: `sanitize(sanitize(x)) == sanitize(x)`. The earlier
/// design rejected `/`, `\`, `\0`, leading dots, and other unsafe
/// characters at validation time; pulling that work into sanitize lets
/// the GUI accept arbitrary user input (drag-and-drop, paste from
/// arbitrary sources) without surfacing path-policy errors to the user.
pub fn sanitize(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_underscore = false;
    for c in input.chars() {
        if is_filename_char(c) {
            // `_` is in the allowed set; the collapse rule applies
            // uniformly to both "originally `_`" and "replacement `_`"
            // so a string like "a___b" or "a   b" both collapse to
            // "a_b".
            if c == '_' {
                if !last_was_underscore {
                    out.push('_');
                    last_was_underscore = true;
                }
            } else {
                out.push(c);
                last_was_underscore = false;
            }
        } else if !last_was_underscore {
            out.push('_');
            last_was_underscore = true;
        }
    }
    out.trim_matches(|c: char| c == '_' || c == '.' || c.is_whitespace())
        .to_string()
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
        assert_eq!(sanitize("wallet.bak"), "wallet.bak");
        assert_eq!(sanitize("my-wallet_2026"), "my-wallet_2026");
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
        let cases = [
            "My Wallet",
            "A  B  C",
            "  wallet  ",
            "wallet",
            "",
            "../evil",
            "..hidden",
            "evil\\name",
            "evil/name",
            "evil\0name",
            "wallet.bak",
            "wallet.",
            "_wallet_",
            "café wallet",
            "C:\\Users\\me\\wallets\\My Wallet",
            "<>:\"/\\|?*",
            "🚀 wallet 🚀",
        ];
        for c in cases {
            let once = sanitize(c);
            let twice = sanitize(&once);
            assert_eq!(once, twice, "not idempotent for input {c:?}");
        }
    }

    #[test]
    fn sanitize_preserves_unicode_letters() {
        assert_eq!(sanitize("café wallet"), "café_wallet");
        assert_eq!(sanitize("日本語 wallet"), "日本語_wallet");
    }

    #[test]
    fn sanitize_replaces_path_separators() {
        assert_eq!(sanitize("../evil"), "evil");
        assert_eq!(sanitize("evil/name"), "evil_name");
        assert_eq!(sanitize("evil\\name"), "evil_name");
        assert_eq!(sanitize("C:\\Users\\me\\wallet"), "C_Users_me_wallet");
        assert_eq!(sanitize("/abs/wallet"), "abs_wallet");
    }

    #[test]
    fn sanitize_replaces_null_bytes() {
        assert_eq!(sanitize("evil\0name"), "evil_name");
        assert_eq!(sanitize("\0\0wallet"), "wallet");
    }

    #[test]
    fn sanitize_strips_leading_dots() {
        // Leading dots would either hide the file on POSIX
        // (`.wallet`) or be parsed as a traversal token (`..`).
        // The post-sanitize result is never a hidden file.
        assert_eq!(sanitize(".hidden"), "hidden");
        assert_eq!(sanitize("..hidden"), "hidden");
        assert_eq!(sanitize("..."), "");
    }

    #[test]
    fn sanitize_strips_trailing_dots() {
        // Windows silently drops trailing dots at file creation; the
        // sanitize step does so explicitly so the on-disk name and
        // the in-memory name agree.
        assert_eq!(sanitize("wallet."), "wallet");
        assert_eq!(sanitize("wallet..."), "wallet");
    }

    #[test]
    fn sanitize_strips_windows_reserved_chars() {
        // `<>:"/\|?*` plus the control range — none survive sanitize.
        assert_eq!(sanitize("a<b>c:d\"e/f\\g|h?i*j"), "a_b_c_d_e_f_g_h_i_j");
    }

    #[test]
    fn sanitize_replaces_emoji() {
        assert_eq!(sanitize("🚀 wallet 🚀"), "wallet");
        assert_eq!(sanitize("rocket🚀wallet"), "rocket_wallet");
    }

    #[test]
    fn sanitize_collapses_underscore_runs() {
        assert_eq!(sanitize("a___b"), "a_b");
        assert_eq!(sanitize("a_b_c"), "a_b_c");
        assert_eq!(sanitize("____"), "");
    }

    #[test]
    fn sanitize_only_unsafe_chars_returns_empty() {
        // All chars stripped → empty result → caller errors at
        // `validate_wallet_name` with "must not be empty".
        assert_eq!(sanitize("////"), "");
        assert_eq!(sanitize("\0\0\0"), "");
        assert_eq!(sanitize("<>?*"), "");
    }

    #[test]
    fn sanitize_output_contains_only_allowed_chars() {
        // Spot check the post-sanitize alphabet across a varied input.
        let s = sanitize("../My 🚀 Wallet/2026\\.bak");
        assert!(
            s.chars().all(is_filename_char),
            "sanitize leaked a forbidden char: {s:?}"
        );
        assert!(!s.starts_with('.'), "leading dot survived: {s:?}");
        assert!(!s.ends_with('.'), "trailing dot survived: {s:?}");
        assert!(!s.starts_with('_'), "leading underscore survived: {s:?}");
        assert!(!s.ends_with('_'), "trailing underscore survived: {s:?}");
    }

    // ── Proptest: sanitize never panics and always produces a safe output ──

    mod prop {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn sanitize_never_panics(s in "\\PC{0,1000}") {
                let _ = sanitize(&s);
            }

            #[test]
            fn sanitize_output_is_filesystem_safe(s in "\\PC{0,500}") {
                let out = sanitize(&s);
                prop_assert!(
                    out.chars().all(is_filename_char),
                    "sanitize allowed forbidden char in output: {:?}",
                    out
                );
                prop_assert!(
                    !out.contains('/'),
                    "sanitize allowed path separator: {:?}",
                    out
                );
                prop_assert!(
                    !out.contains('\\'),
                    "sanitize allowed backslash: {:?}",
                    out
                );
                prop_assert!(
                    !out.contains('\0'),
                    "sanitize allowed null byte: {:?}",
                    out
                );
                if !out.is_empty() {
                    prop_assert!(
                        !out.starts_with('.'),
                        "sanitize allowed leading dot: {:?}",
                        out
                    );
                    prop_assert!(
                        !out.starts_with('_'),
                        "sanitize allowed leading underscore: {:?}",
                        out
                    );
                }
            }

            #[test]
            fn sanitize_is_idempotent_proptest(s in "\\PC{0,500}") {
                let once = sanitize(&s);
                let twice = sanitize(&once);
                prop_assert_eq!(once, twice);
            }
        }
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
