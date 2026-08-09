// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Drainable-`P` read projection (DS-PR-3 PR-B; F-D2 aggregate).
//!
//! Single serializable DTO for the Tauri wire — session and command share
//! this type so there is no identity hop through `commands.rs`. Same
//! ownership pattern as [`crate::staking_view`] and [`crate::transfer_history`].
//!
//! Two-armed on purpose (rule 82): `Ready` carries the anchored aggregate
//! spendable scalar; `Syncing` is the transient anchor arm (render a
//! placeholder, never a zero). A non-transient fault is *not* a variant —
//! it stays `Err(String)` on the session method so a bad read never
//! masquerades as a value.

use serde::Serialize;

/// Drainable-`P` read result on the wire (and session boundary).
///
/// Internally tagged so the frontend matches on `status`. No `Clone`: no
/// caller needs a second copy (rule 21).
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DrainBalance {
    /// Anchored aggregate spendable `P`, atomic units. Display-only: the
    /// figure is rendered, never fed to transaction arithmetic (a real drain
    /// computes its amounts in core Rust `u64`), and the SKL formatter is
    /// coarser than the JS `number` ULP across the whole supply range, so the
    /// `u64` > 2^53 serialization gap is not observable in the rendered
    /// value. Sibling of `Balance` fields for the same reason. If this figure
    /// ever seeds a tx amount, the whole balance pipeline migrates to
    /// string+BigInt (FOLLOWUPS: "Atomic amounts serialized as JS number").
    Ready { spendable: u64 },
    /// Transient: send-path reference not yet anchorable. `detail` is static
    /// operator text — no amount, no gindex.
    Syncing { detail: String },
}
