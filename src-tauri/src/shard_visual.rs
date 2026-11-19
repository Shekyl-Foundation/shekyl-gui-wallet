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
use shekyl_shard_visual::fixtures;
use shekyl_shard_visual::{
    parameters_from_aggregate, parameters_with_hash_override, recipe_from_params,
    render_candidate_png_from_params, CandidateRecipe, ShardAggregate, VisualError,
    RENDER_REVISION,
};
use tauri::State;

use crate::account_root::AccountRoot;
use crate::state::AppState;

pub(crate) const MIN_SIZE: u32 = 64;
pub(crate) const MAX_SIZE: u32 = 512;
pub(crate) const DEFAULT_SIZE: u32 = 192;

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
            dominant_regime: f.dominant_regime.clone(),
            shard_hash: hex::encode(f.aggregate.shard_hash),
        })
        .collect()
}

#[tauri::command]
pub fn render_shard_preview(
    state: State<'_, AppState>,
    request: RenderShardPreviewRequest,
) -> Result<RenderShardPreviewResponse, String> {
    let fixture = fixtures::by_id(&request.fixture_id)
        .ok_or_else(|| format!("unknown fixture {:?}", request.fixture_id))?;

    let size = request
        .size
        .unwrap_or(DEFAULT_SIZE)
        .clamp(MIN_SIZE, MAX_SIZE);
    let hash_override = parse_hash_override(request.hash_override.as_deref())?;

    let cache_key = cache_digest(
        &fixture.id,
        fixture.aggregate.shard_hash,
        hash_override,
        size,
    );
    let recipe = recipe_for(&fixture.aggregate, hash_override);
    let cache = cache_dir_in(&state.root);
    let png = render_cached(
        cache.as_deref(),
        &cache_key,
        &fixture.aggregate,
        hash_override,
        size,
    )
    .map_err(|e| e.to_string())?;

    Ok(RenderShardPreviewResponse {
        png_base64: STANDARD.encode(&png),
        recipe,
        cache_key,
        label: fixture.label.clone(),
    })
}

/// Return the cached PNG for `cache_key` in `cache`, rendering and
/// persisting it on miss. With no cache directory (no per-account root:
/// [`cache_dir_in`]) the PNG is rendered and nothing is kept.
///
/// Concurrent renders for the same key can race on the write: both miss the
/// cache, both render, and the second `rename` may fail (especially on
/// platforms where rename refuses an existing destination). In that case the
/// other writer already produced a valid file — read it back rather than
/// failing a successful render.
pub(crate) fn render_cached(
    cache: Option<&Path>,
    cache_key: &str,
    aggregate: &ShardAggregate,
    hash_override: Option<[u8; 32]>,
    size: u32,
) -> Result<Vec<u8>, String> {
    let Some(dir) = cache else {
        return render_png(aggregate, hash_override, size).map_err(|e| e.to_string());
    };
    if let Some(png) = read_cache(dir, cache_key, size)? {
        return Ok(png);
    }
    let png = render_png(aggregate, hash_override, size).map_err(|e| e.to_string())?;
    match write_cache(dir, cache_key, size, &png) {
        Ok(()) => Ok(png),
        Err(write_err) => match read_cache(dir, cache_key, size)? {
            Some(cached) => Ok(cached),
            None => Err(write_err),
        },
    }
}

pub(crate) fn recipe_for(
    aggregate: &ShardAggregate,
    hash_override: Option<[u8; 32]>,
) -> CandidateRecipe {
    let params = if let Some(hash) = hash_override {
        parameters_with_hash_override(aggregate, hash)
    } else {
        parameters_from_aggregate(aggregate)
    };
    recipe_from_params(&params)
}

fn render_png(
    aggregate: &ShardAggregate,
    hash_override: Option<[u8; 32]>,
    size: u32,
) -> Result<Vec<u8>, VisualError> {
    // The override goes through the crate's own constructor so the PNG's
    // provenance chunks and the recipe agree (canonical=false); swapping
    // the hash on a cloned aggregate rendered the override as canonical.
    let params = if let Some(hash) = hash_override {
        parameters_with_hash_override(aggregate, hash)
    } else {
        parameters_from_aggregate(aggregate)
    };
    render_candidate_png_from_params(&params, size)
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
    Ok(Some(bytes.try_into().map_err(|_| {
        "hash_override must be 32 bytes (64 hex characters)".to_string()
    })?))
}

/// LOAD-BEARING, NOT AN OPTIMIZATION — do not remove this cache.
///
/// Measured on the rule-76 floor device (Raspberry Pi 4, shekyl-core
/// `docs/benchmarks/shard_visual_budget_matrix_pi4_20260906T090000Z.txt`), a
/// single candidate.v1 render costs a median of **69–164 ms at 128px** and
/// **155–385 ms at 256px**. The 2026-09-06 amendment to *Performance targets*
/// (shekyl-core `docs/V3_SHARD_VISUALIZATION.md`) states the consequence
/// explicitly: the thumbnail and portfolio tiers stay interactive **only**
/// because this cache makes the cost once per shard per `RENDER_REVISION`
/// rather than once per view. Rendering per view on the floor device is a
/// visibly slow portfolio, which is the failure the spec's UX rules forbid.
///
/// The comment lives here, at the site, and not only in the design doc,
/// because a design doc does not defend a line of code from a cleanup PR.
pub(crate) fn cache_digest(
    id: &str,
    base_hash: [u8; 32],
    hash_override: Option<[u8; 32]>,
    size: u32,
) -> String {
    let hash = hash_override.unwrap_or(base_hash);
    let mut hasher = Sha256::new();
    // RENDER_REVISION invalidates cached PNGs when the crate's pixel
    // derivation changes under the same spec version (shekyl-core #617).
    hasher.update(b"shard-visual-v1");
    hasher.update(RENDER_REVISION.to_le_bytes());
    hasher.update(hash);
    hasher.update(id.as_bytes());
    hasher.update(size.to_le_bytes());
    hex::encode(hasher.finalize())
}

/// The render cache's directory's name in the GUI's directory in the
/// per-account root.
pub(crate) const CACHE_DIR_NAME: &str = "shard-visual";

/// The render cache: [`CACHE_DIR_NAME`] in the GUI's directory in the
/// per-account root ([`AccountRoot::settings_dir`]; core rule 83 IG-13),
/// never a cache folder found through `XDG_CACHE_HOME` or `HOME`, which
/// the regenesis upgrade does not clear. `None` when the start left no
/// root (no home, or a root it could not make): the PNG is then rendered
/// on every view and nothing is kept.
pub(crate) fn cache_dir_in(root: &AccountRoot) -> Option<PathBuf> {
    root.settings_dir().ok().map(|dir| dir.join(CACHE_DIR_NAME))
}

fn cache_path(dir: &Path, key: &str, size: u32) -> PathBuf {
    dir.join(format!("{key}_{size}.png"))
}

fn read_cache(dir: &Path, key: &str, size: u32) -> Result<Option<Vec<u8>>, String> {
    let path = cache_path(dir, key, size);
    if !path.is_file() {
        return Ok(None);
    }
    fs::read(&path).map(Some).map_err(|e| e.to_string())
}

fn write_cache(dir: &Path, key: &str, size: u32, png: &[u8]) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    atomic_write(&cache_path(dir, key, size), png)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("png.part");
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The render cache is in the GUI's directory in the per-account root
    /// (core rule 83 IG-13), and there is none without a root. Red edit:
    /// the old `app_cache_dir()` (`~/.cache/org.shekyl.wallet`, found
    /// through `XDG_CACHE_HOME` or `HOME`) fails the first assertion.
    #[test]
    fn the_render_cache_is_in_the_per_account_root_or_nowhere() {
        let root = std::env::temp_dir().join("shekyl-gui-shard-root");
        assert_eq!(
            cache_dir_in(&AccountRoot::Ready(root.clone())),
            Some(root.join("shekyl-gui-wallet").join("shard-visual"))
        );
        assert_eq!(cache_dir_in(&AccountRoot::NoHome), None);
        assert_eq!(cache_dir_in(&AccountRoot::not_made_at(root)), None);
    }

    /// With a cache directory a render is kept there and read back; with
    /// none it is drawn all the same and nothing is kept, so a wallet
    /// whose root could not be made still draws its cards.
    #[test]
    fn a_render_is_kept_in_its_cache_and_drawn_without_one() {
        let fixture = fixtures::all().into_iter().next().expect("a fixture");
        let key = cache_digest(&fixture.id, fixture.aggregate.shard_hash, None, MIN_SIZE);
        let base =
            std::env::temp_dir().join(format!("shekyl-gui-shard-cache-{}", std::process::id()));
        let dir = base.join(CACHE_DIR_NAME);

        let drawn = render_cached(None, &key, &fixture.aggregate, None, MIN_SIZE).expect("drawn");
        assert!(!drawn.is_empty());
        assert!(!base.exists(), "nothing is kept without a cache");

        let kept = render_cached(Some(&dir), &key, &fixture.aggregate, None, MIN_SIZE)
            .expect("drawn and kept");
        assert_eq!(kept, drawn);
        let path = cache_path(&dir, &key, MIN_SIZE);
        assert_eq!(fs::read(&path).expect("kept"), drawn);
        // A second view reads the kept file, not a fresh render.
        fs::write(&path, b"kept").unwrap();
        assert_eq!(
            render_cached(Some(&dir), &key, &fixture.aggregate, None, MIN_SIZE).unwrap(),
            b"kept"
        );
        fs::remove_dir_all(&base).unwrap();
    }
}
