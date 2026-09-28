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

use crate::wire::AtomicUnitsString;

/// Drainable-`P` read result on the wire (and session boundary).
///
/// Internally tagged so the frontend matches on `status`. No `Clone`: no
/// caller needs a second copy (rule 21).
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DrainBalance {
    /// Anchored aggregate spendable `P`, atomic units — a decimal string on
    /// the wire like every atomic amount (`wire::AtomicUnitsString`), so the
    /// figure a `stake return` is sized against is never rounded by a JS
    /// `number`.
    Ready { spendable: AtomicUnitsString },
    /// Transient: send-path reference not yet anchorable. `detail` is static
    /// operator text — no amount, no gindex.
    Syncing { detail: String },
}
