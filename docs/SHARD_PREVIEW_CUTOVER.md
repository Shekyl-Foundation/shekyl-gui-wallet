# Shard preview cutover (ArchivalEngine Stage 5)

This document records how the GUI wallet's **Shard Identity Preview** on the
Staking tab transitions from beta fixtures to production archival shards.

## Current state (pre-Stage 5)

| Surface | Implementation |
|---------|------------------|
| Rust renderer | `shekyl-core/rust/shekyl-shard-visual` — candidate.v1 compositor |
| Tauri commands | `list_shard_preview_fixtures`, `render_shard_preview` in `src-tauri/src/shard_visual.rs` |
| UI | `src/components/staking/ShardIdentityPreview.tsx` on `Staking.tsx` |
| Data source | Embedded regime fixtures (`shekyl-shard-visual::fixtures`) |
| Cache | `{app_cache}/shard-visual/{digest}_{size}.png` |

Fixtures mirror the visualization explorer fake-chain shards 0–5 (genesis through
whale regimes). Optional `hash_override` on render requests exercises palette and
opacity variation without changing aggregate features.

## Stage 5 cutover checklist

When `ArchivalEngine` lands and the wallet can list real archived shards:

1. **Replace fixture list source**
   - Change `list_shard_preview_fixtures` to call ArchivalEngine (or wallet RPC
     wrapping it) and map live `ShardAggregate` + `content_hash` into
     `ShardPreviewFixtureInfo`.
   - Keep the command name stable so the React layer needs minimal changes.

2. **Render from live aggregates**
   - `render_shard_preview` should accept either a fixture id (dev/regtest) or a
     production shard id / content hash once archival state exists.
   - Cache key must include `content_hash` (not fixture id) so identical shards
     hit disk cache across sessions.

3. **UI placement**
   - Staking tab preview remains the pre-stake identity affordance.
   - Consider an **Archives** sidebar tab when users manage multiple archived
     shards; link from Staking preview to full shard detail there.

4. **Remove beta disclaimer**
   - Drop the amber "Pre-archival preview only" banner in
     `ShardIdentityPreview.tsx` once renders are backed by chain state.

5. **Delete fixture-only paths**
   - After cutover, remove or gate `shekyl-shard-visual::fixtures` from production
     builds if no caller remains (keep for dev/regtest behind a feature flag if
     useful).

## Stable integration contracts

These types are the boundary between UI and backend; preserve them through cutover:

- `ShardPreviewFixtureInfo` — id, label, dominant_regime, shard_hash (hex)
- `RenderShardPreviewRequest` — fixture_id, optional hash_override, size
- `RenderShardPreviewResponse` — png_base64, recipe, cache_key, label

## Related specs

- `shekyl-core/docs/V3_SHARD_VISUALIZATION.md` — candidate.v1 compositor spec
- `shekyl-dev/visualization/` — Python explorer reference implementation
