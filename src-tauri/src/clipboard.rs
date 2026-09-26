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

//! Clipboard lifecycle for the recovery phrase — a feature module (rule 27),
//! kept out of `commands.rs` so the god-file does not grow.
//!
// Rust owns the clipboard lifecycle for the recovery phrase. The create page
// hands the text to `copy_to_clipboard`; Rust writes it and keeps ONLY a hash
// of what it wrote. `clear_clipboard` then clears only while the clipboard
// still holds exactly that — so a value the user copied in another app since
// is never destroyed (a clear keyed on "we copied the seed at some point"
// would have). Done from Rust, not the webview, because a webview
// `writeText` can be refused once the window loses focus — precisely when the
// user has alt-tabbed to paste — and because the plugin's Rust API consults no
// permission scope, so the webview is granted no clipboard capability at all.
//
// Both commands are `async`: the plugin documents that `read_text` must not
// run on the main thread (it can deadlock on Linux when reading back text
// this very app wrote), so the I/O runs under `spawn_blocking`.
use tauri::State;

use crate::state::AppState;

/// Whether the clipboard's current text is still the text this app wrote.
pub fn clipboard_still_ours(owned: Option<[u8; 32]>, current: &str) -> bool {
    owned.is_some_and(|h| h == sha256_of(current))
}

fn sha256_of(text: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(text.as_bytes()).into()
}

#[tauri::command]
pub async fn copy_to_clipboard(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    text: String,
) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    // Own the secret in a wiping container from the first line; the IPC
    // buffer it arrived in is the webview edge's and not ours to zero.
    let text = zeroize::Zeroizing::new(text);
    let hash = sha256_of(&text);
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        handle
            .clipboard()
            .write_text(text.as_str())
            .map_err(|e| format!("copy to clipboard: {e}"))
    })
    .await
    .map_err(|e| format!("clipboard task: {e}"))??;
    *state.clipboard_owned.lock().await = Some(hash);
    Ok(())
}

/// Clear the clipboard if — and only if — it still holds what
/// `copy_to_clipboard` last wrote. Returns whether anything was cleared.
/// Whatever happens, this app stops tracking the clipboard afterwards.
#[tauri::command]
pub async fn clear_clipboard(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let owned = state.clipboard_owned.lock().await.take();
    if owned.is_none() {
        return Ok(false);
    }
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let clipboard = handle.clipboard();
        // An unreadable clipboard cannot be proven ours: leave it alone.
        let Ok(current) = clipboard.read_text() else {
            return Ok(false);
        };
        if !clipboard_still_ours(owned, &current) {
            return Ok(false);
        }
        clipboard
            .clear()
            .map(|()| true)
            .map_err(|e| format!("clear clipboard: {e}"))
    })
    .await
    .map_err(|e| format!("clipboard task: {e}"))?
}

#[cfg(test)]
mod clipboard_tests {
    use super::{clipboard_still_ours, sha256_of};

    #[test]
    fn clears_only_what_it_wrote() {
        let owned = Some(sha256_of("word1 word2 word3"));
        assert!(clipboard_still_ours(owned, "word1 word2 word3"));
        assert!(!clipboard_still_ours(
            owned,
            "something the user copied later"
        ));
        assert!(!clipboard_still_ours(owned, ""));
    }

    #[test]
    fn nothing_tracked_means_nothing_is_ours() {
        assert!(!clipboard_still_ours(None, "word1 word2 word3"));
        assert!(!clipboard_still_ours(None, ""));
    }
}
