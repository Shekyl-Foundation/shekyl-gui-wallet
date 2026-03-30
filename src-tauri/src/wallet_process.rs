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

//! Lifecycle management for the shekyl-wallet-rpc child process.
//!
//! Binary resolution order:
//!   1. Tauri sidecar (production — bundled via `externalBin`)
//!   2. PATH lookup   (development — system-installed binary)
//!   3. User-configured override (Settings page)
//!
//! The module spawns wallet-rpc in `--wallet-dir` mode so that wallets can be
//! created, opened, and closed via JSON-RPC without restarting the process.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use reqwest::Client;

use crate::wallet_rpc;

const BINARY_NAME: &str = "shekyl-wallet-rpc";
const READY_POLL_INTERVAL: Duration = Duration::from_millis(300);
const READY_TIMEOUT: Duration = Duration::from_secs(15);

/// Resolves the wallet-rpc binary path using a layered strategy.
///
/// `user_override` takes highest precedence when set. Falls back to PATH.
/// Sidecar resolution is handled by Tauri's runtime when `externalBin` is
/// configured; this function covers dev and user-override cases.
pub fn resolve_binary(user_override: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(p) = user_override {
        if p.exists() {
            return Ok(p.to_path_buf());
        }
        return Err(format!(
            "Configured wallet-rpc binary not found: {}",
            p.display()
        ));
    }

    // PATH lookup via `which`
    if let Ok(path) = which::which(BINARY_NAME) {
        return Ok(path);
    }

    // Platform-specific install directories
    let candidates = platform_binary_candidates();
    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    Err(format!(
        "Could not find {BINARY_NAME}. Install Shekyl or set the binary path in Settings."
    ))
}

fn platform_binary_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(target_os = "linux")]
    {
        paths.push(PathBuf::from("/usr/local/bin").join(BINARY_NAME));
        paths.push(PathBuf::from("/usr/bin").join(BINARY_NAME));
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join(".local/bin").join(BINARY_NAME));
        }
    }

    #[cfg(target_os = "macos")]
    {
        paths.push(PathBuf::from("/usr/local/bin").join(BINARY_NAME));
        paths.push(PathBuf::from("/opt/homebrew/bin").join(BINARY_NAME));
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join(".local/bin").join(BINARY_NAME));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(program_files) = std::env::var_os("ProgramFiles") {
            paths.push(PathBuf::from(program_files).join("Shekyl").join(format!("{BINARY_NAME}.exe")));
        }
        if let Some(local) = dirs::data_local_dir() {
            paths.push(local.join("Shekyl").join(format!("{BINARY_NAME}.exe")));
        }
    }

    paths
}

/// Spawns `shekyl-wallet-rpc` in `--wallet-dir` mode.
pub fn spawn_wallet_rpc(
    binary: &Path,
    wallet_dir: &Path,
    daemon_url: &str,
    rpc_port: u16,
) -> Result<Child, String> {
    std::fs::create_dir_all(wallet_dir).map_err(|e| {
        format!(
            "Failed to create wallet directory {}: {e}",
            wallet_dir.display()
        )
    })?;

    // Strip /json_rpc suffix from daemon URL — wallet-rpc expects host:port
    let daemon_address = daemon_url
        .trim_end_matches("/json_rpc")
        .trim_end_matches('/')
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .to_string();

    let child = Command::new(binary)
        .arg("--wallet-dir")
        .arg(wallet_dir)
        .arg("--rpc-bind-port")
        .arg(rpc_port.to_string())
        .arg("--daemon-address")
        .arg(&daemon_address)
        .arg("--disable-rpc-login")
        .arg("--non-interactive")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start {BINARY_NAME}: {e}"))?;

    Ok(child)
}

/// Polls `get_version` until the wallet-rpc is accepting connections.
pub async fn wait_for_ready(client: &Client, rpc_url: &str) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + READY_TIMEOUT;

    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(format!(
                "{BINARY_NAME} did not become ready within {}s",
                READY_TIMEOUT.as_secs()
            ));
        }

        match wallet_rpc::get_version(client, rpc_url).await {
            Ok(_) => return Ok(()),
            Err(_) => tokio::time::sleep(READY_POLL_INTERVAL).await,
        }
    }
}

/// Graceful shutdown: try RPC `stop_wallet`, then SIGTERM, then SIGKILL.
pub async fn shutdown(client: &Client, rpc_url: &str, child: &mut Child) {
    // Try graceful RPC shutdown first
    let _ = wallet_rpc::stop_wallet(client, rpc_url).await;

    // Give it a moment to exit
    tokio::time::sleep(Duration::from_secs(2)).await;

    // If still running, escalate
    match child.try_wait() {
        Ok(Some(_)) => {} // already exited
        _ => {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Check if a process is still running.
pub fn is_running(child: &mut Child) -> bool {
    matches!(child.try_wait(), Ok(None))
}

/// Check if the wallet-rpc port is already in use (stale process detection).
pub async fn port_is_occupied(client: &Client, rpc_url: &str) -> bool {
    wallet_rpc::get_version(client, rpc_url).await.is_ok()
}
