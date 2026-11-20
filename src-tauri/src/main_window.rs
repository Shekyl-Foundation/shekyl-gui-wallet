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

//! The app's one window, built in code so its web view keeps nothing
//! outside the per-account root (core rule 83 IG-13).
//!
//! A window in `tauri.conf.json` can name its web view's data directory
//! only relative to Tauri's app-data folder (`org.shekyl.wallet`, found
//! through `XDG_*` or the platform's known folders), so the window is made
//! here instead. Its web view is incognito: the page keeps no web storage,
//! so there is nothing for it to keep. On Linux (WebKitGTK) and macOS
//! (WKWebView) that is an in-memory store and nothing is written; WebView2
//! on Windows still needs a user-data folder for its browser process,
//! which is [`WEBVIEW_DIR_NAME`] in the GUI's settings directory.

use std::path::PathBuf;

use tauri::{WebviewUrl, WebviewWindowBuilder};

use crate::account_root::AccountRoot;

/// The window's label; `capabilities/default.json` grants the commands to
/// it by this name.
pub const MAIN_WINDOW: &str = "main";

/// The web view's data folder's name in the GUI's settings directory.
pub const WEBVIEW_DIR_NAME: &str = "webview";

/// The web view's data folder in `root`: [`WEBVIEW_DIR_NAME`] in the GUI's
/// settings directory, or `None` without a root.
pub fn webview_dir_in(root: &AccountRoot) -> Option<PathBuf> {
    root.settings_dir()
        .ok()
        .map(|dir| dir.join(WEBVIEW_DIR_NAME))
}

/// Opens the window: incognito, with its web view's data in `data_dir`.
/// Without one (no root) Tauri gives WebView2 its own app-data folder on
/// Windows, the one place the GUI writes outside the root; it needs a
/// folder to show the window that says why there is no root.
pub fn open(app: &tauri::App, data_dir: Option<PathBuf>) -> tauri::Result<()> {
    let mut window = WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::default())
        .title("Shekyl Wallet")
        .inner_size(1024.0, 768.0)
        .min_inner_size(800.0, 600.0)
        .resizable(true)
        .center()
        .incognito(true);
    if let Some(dir) = data_dir {
        window = window.data_directory(dir);
    }
    window.build()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The web view's data folder is in the GUI's settings directory in the
    /// per-account root, and there is none without a root. Red edit:
    /// Tauri's own folder (`app_local_data_dir`, `%LOCALAPPDATA%\org.shekyl.wallet`)
    /// fails here.
    #[test]
    fn the_webview_data_folder_is_in_the_per_account_root_or_nowhere() {
        let root = PathBuf::from("root");
        assert_eq!(
            webview_dir_in(&AccountRoot::Ready(root.clone())),
            Some(root.join("shekyl-gui-wallet").join("webview"))
        );
        assert_eq!(webview_dir_in(&AccountRoot::NoHome), None);
        assert_eq!(
            webview_dir_in(&AccountRoot::not_made_at(root.clone())),
            None
        );
    }
}
