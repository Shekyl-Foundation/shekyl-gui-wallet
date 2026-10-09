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

## Shards page (operator gallery)

The sidebar **Shards** page is not this Staking-tab preview. It speaks two
commands:

- `list_shards` (app shell, `command_surface.conf`) → daemon
  `get_archival_shard_coverage` (no Tor, no bodies). `expected_profit_atomic`
  on each row is a decimal string of atomic units so values above 2^53 stay
  exact; the gallery sums and formats with BigInt.
- `get_shard_view` — the wallet contract's method of that name
  (`wallet_rpc.yaml` 0.11.0, shekyl-core `docs/design/SHARD_VIEW_FETCH.md`
  SV-D), adapted in `src-tauri/src/shard_coverage.rs`. The wallet's daemon
  **fetches the shard's archival body from a holder**, verifies it, and
  answers the aggregate (`ShardView`: counts, `shard_hash` — the SV-D1 view
  hash, a fold over the archival bytes, distinct from the challenge hash —
  `archival_len`, `time_range_seconds`, `close_height`). The GUI draws
  candidate.v1 from that aggregate with `shekyl-shard-visual` and caches the
  PNG under `{app_cache}/shard-visual/` keyed on shard id, view hash and
  size (a reorg that rewrites the shard changes the hash, so it misses); the
  wire type is `ShardViewRender { view, png_base64, recipe, cache_key }`.
  The command fails closed if the reply's `shard_id` does not match the
  request, or its hash is not 32 bytes of hex: both are
  `DAEMON_PROTOCOL_VIOLATION`, never a render. Every view request is a real
  fetch on the network — that traffic is the point — so the card asks only
  when it is visible or selected, never on page mount.

The contract's three refusals are each a card state (rule 82), read off the
`ContractError.code` the command rejects with: `SHARD_STILL_OPEN` (the shard
is still being written; a clock), `SHARD_UNAVAILABLE` (no holder served it
this time; a retry button that does not toggle selection),
`SHARD_VIEW_NOT_OFFERED` (`data.cause` is `restricted` — the wallet's daemon
serves the view only on its unrestricted listener — or `skeleton_absent`).
Anything else is a fault with the wallet's sentence as the title. No state
is an empty frame, and a refused view is never cached.

The GUI never fetches shard bodies itself. Selection is session state (you
pick; the network does not assign). Picks that leave the latest coverage
list are dropped. The gallery panel (`ShardCoverageGallery`) owns the
coverage fetch and fail-closed render; it mounts a window of cards
(`GALLERY_PAGE_SIZE`) with Show more, so a long frozen set does not create
one observer per row. The fixture preview on the Staking tab is unchanged
until the Stage 5 checklist below.

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
