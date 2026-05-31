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

//! Tauri bridge for live shard identity previews on the Staking tab.
//!
//! Pre-ArchivalEngine (Stage 5), renders use embedded regime fixtures from
//! `shekyl-shard-visual`. See `docs/SHARD_PREVIEW_CUTOVER.md` for the
//! production cutover hooks.

use std::fs;
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shekyl_shard_visual::fixtures::{self, PreviewFixture};
use shekyl_shard_visual::{
    parameters_from_aggregate, parameters_with_hash_override, recipe_from_params,
    render_candidate_png, CandidateRecipe, VisualError,
};
use tauri::{AppHandle, Manager};

const MIN_SIZE: u32 = 64;
const MAX_SIZE: u32 = 512;
const DEFAULT_SIZE: u32 = 192;

#[derive(Debug, Serialize)]
pub struct ShardPreviewFixtureInfo {
    pub id: String,
    pub label: String,
    pub dominant_regime: String,
    pub shard_hash: String,
}

#[derive(Debug, Deserialize)]
pub struct RenderShardPreviewRequest {
    pub fixture_id: String,
    /// Optional 64-char hex hash replacing the fixture's shard_hash for exploration.
    pub hash_override: Option<String>,
    pub size: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct RenderShardPreviewResponse {
    pub png_base64: String,
    pub recipe: CandidateRecipe,
    pub cache_key: String,
    pub label: String,
}

#[tauri::command]
pub fn list_shard_preview_fixtures() -> Vec<ShardPreviewFixtureInfo> {
    fixtures::all()
        .into_iter()
        .map(|f| ShardPreviewFixtureInfo {
            id: f.id.clone(),
            label: f.label.clone(),
            dominant_regime: f.aggregate.dominant_regime.clone(),
            shard_hash: hex::encode(f.aggregate.shard_hash),
        })
        .collect()
}

#[tauri::command]
pub fn render_shard_preview(
    app: AppHandle,
    request: RenderShardPreviewRequest,
) -> Result<RenderShardPreviewResponse, String> {
    let fixture = fixtures::by_id(&request.fixture_id)
        .ok_or_else(|| format!("unknown fixture {:?}", request.fixture_id))?;

    let size = request.size.unwrap_or(DEFAULT_SIZE).clamp(MIN_SIZE, MAX_SIZE);
    let hash_override = parse_hash_override(request.hash_override.as_deref())?;

    let cache_key = cache_digest(&fixture, hash_override, size);
    if let Some(png) = read_cache(&app, &cache_key, size)? {
        let recipe = recipe_for(&fixture, hash_override);
        return Ok(RenderShardPreviewResponse {
            png_base64: STANDARD.encode(&png),
            recipe,
            cache_key,
            label: fixture.label.clone(),
        });
    }

    let png = render_png(&fixture, hash_override, size).map_err(|e| e.to_string())?;
    write_cache(&app, &cache_key, size, &png)?;
    let recipe = recipe_for(&fixture, hash_override);

    Ok(RenderShardPreviewResponse {
        png_base64: STANDARD.encode(&png),
        recipe,
        cache_key,
        label: fixture.label.clone(),
    })
}

fn recipe_for(fixture: &PreviewFixture, hash_override: Option<[u8; 32]>) -> CandidateRecipe {
    let params = if let Some(hash) = hash_override {
        parameters_with_hash_override(&fixture.aggregate, hash)
    } else {
        parameters_from_aggregate(&fixture.aggregate)
    };
    recipe_from_params(&params)
}

fn render_png(
    fixture: &PreviewFixture,
    hash_override: Option<[u8; 32]>,
    size: u32,
) -> Result<Vec<u8>, VisualError> {
    let mut agg = fixture.aggregate.clone();
    if let Some(hash) = hash_override {
        agg.shard_hash = hash;
    }
    render_candidate_png(&agg, size)
}

fn parse_hash_override(raw: Option<&str>) -> Result<Option<[u8; 32]>, String> {
    let Some(s) = raw else {
        return Ok(None);
    };
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let bytes = hex::decode(trimmed).map_err(|e| format!("invalid hash_override hex: {e}"))?;
    Ok(Some(
        bytes
            .try_into()
            .map_err(|_| "hash_override must be 32 bytes (64 hex characters)".to_string())?,
    ))
}

fn cache_digest(
    fixture: &PreviewFixture,
    hash_override: Option<[u8; 32]>,
    size: u32,
) -> String {
    let hash = hash_override.unwrap_or(fixture.aggregate.shard_hash);
    let mut hasher = Sha256::new();
    hasher.update(b"shard-visual-v1");
    hasher.update(hash);
    hasher.update(fixture.id.as_bytes());
    hasher.update(size.to_le_bytes());
    hex::encode(hasher.finalize())
}

fn cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_cache_dir()
        .map(|p| p.join("shard-visual"))
        .map_err(|e| e.to_string())
}

fn cache_path(app: &AppHandle, key: &str, size: u32) -> Result<PathBuf, String> {
    Ok(cache_dir(app)?.join(format!("{key}_{size}.png")))
}

fn read_cache(app: &AppHandle, key: &str, size: u32) -> Result<Option<Vec<u8>>, String> {
    let path = cache_path(app, key, size)?;
    if !path.is_file() {
        return Ok(None);
    }
    fs::read(&path).map(Some).map_err(|e| e.to_string())
}

fn write_cache(app: &AppHandle, key: &str, size: u32, png: &[u8]) -> Result<(), String> {
    let dir = cache_dir(app)?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = cache_path(app, key, size)?;
    atomic_write(&path, png)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("png.part");
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}
