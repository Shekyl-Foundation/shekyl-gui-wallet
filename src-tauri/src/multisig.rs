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

//! PQC multisig — compiled only under the `multisig` cargo feature.
//!
//! Alpha posture (ruled 2026-09-25): this code is kept, not deleted, and it is
//! not enabled. The V3.1 multisig round (`shekyl-multisig`, Track B → Option E′)
//! has not ruled a transport, and the file-shuttle shape below — a signing
//! request out to a path, a signature back in from one — is the same shape
//! the wallet rejected outright for cold signing on 2026-09-07. It reaches
//! users through the wallet contract when a round rules it, and the GUI then
//! consumes that like any other method. Until then the commands, the page,
//! the nav entry and the Help section all follow this one feature switch,
//! which the frontend reads via `get_feature_flags`.
//!
//! The group operations ran only on the retired Wallet2 backend and return an
//! honest refusal; the file primitives are real reads/writes of caller-named
//! paths, which is why they are not compiled into a default build.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::ENGINE_BACKEND_UNSUPPORTED;
use crate::state::AppState;

#[tauri::command]
pub async fn create_multisig_group(
    _state: State<'_, AppState>,
    _n_total: u8,
    _m_required: u8,
    _participant_keys: Vec<String>,
) -> Result<serde_json::Value, String> {
    Err(ENGINE_BACKEND_UNSUPPORTED.into())
}

#[tauri::command]
pub async fn get_multisig_info(_state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    Err(ENGINE_BACKEND_UNSUPPORTED.into())
}

#[tauri::command]
pub async fn sign_multisig_partial(
    _state: State<'_, AppState>,
    _signing_request: String,
) -> Result<serde_json::Value, String> {
    Err(ENGINE_BACKEND_UNSUPPORTED.into())
}

// ─── Group Descriptor import/export ──────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupDescriptorPayload {
    pub version: u8,
    pub group_id: String,
    pub m_required: u8,
    pub n_total: u8,
    pub spend_auth_version: u8,
    pub participant_pubkeys: Vec<String>,
    pub address_fingerprint: String,
    pub relays: Vec<GroupDescriptorRelay>,
    pub created_at: u64,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupDescriptorRelay {
    pub url: String,
    pub operator_id: String,
}

#[tauri::command]
pub async fn export_group_descriptor(
    _state: State<'_, AppState>,
    _path: String,
) -> Result<(), String> {
    // Depends on multisig group info, which is Wallet2-only and retired.
    Err(ENGINE_BACKEND_UNSUPPORTED.into())
}

#[tauri::command]
pub async fn import_group_descriptor(path: String) -> Result<GroupDescriptorPayload, String> {
    let json = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read descriptor file: {e}"))?;
    let desc: GroupDescriptorPayload =
        serde_json::from_str(&json).map_err(|e| format!("Invalid descriptor format: {e}"))?;

    if desc.version != 1 {
        return Err(format!("Unsupported descriptor version: {}", desc.version));
    }
    if desc.m_required == 0 || desc.m_required > desc.n_total {
        return Err(format!(
            "Invalid threshold: {}-of-{}",
            desc.m_required, desc.n_total
        ));
    }
    if desc.participant_pubkeys.len() != desc.n_total as usize {
        return Err(format!(
            "Expected {} pubkeys, got {}",
            desc.n_total,
            desc.participant_pubkeys.len()
        ));
    }

    Ok(desc)
}

// ─── File-based transport ────────────────────────────────────────────────────

#[tauri::command]
pub async fn export_signing_request_file(
    state: State<'_, AppState>,
    signing_request: String,
    path: String,
) -> Result<(), String> {
    let _ = &state;
    std::fs::write(&path, signing_request.as_bytes())
        .map_err(|e| format!("Failed to write signing request: {e}"))
}

#[tauri::command]
pub async fn import_signing_request_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("Failed to read signing request: {e}"))
}

#[tauri::command]
pub async fn export_signature_response_file(response: String, path: String) -> Result<(), String> {
    std::fs::write(&path, response.as_bytes())
        .map_err(|e| format!("Failed to write signature response: {e}"))
}
