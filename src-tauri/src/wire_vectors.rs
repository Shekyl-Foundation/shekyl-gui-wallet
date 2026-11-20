// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Core's wire vectors, for this crate's tests.
//!
//! The wallet reads the daemon through core's types, and its tests read
//! core's own hand-written replies (`shekyl-rpc-types/tests/vectors/rpc`),
//! from the same checkout the path-deps compile against. No copy lives
//! here, so a reply the vectors change is a test that changes with it.

use std::path::PathBuf;

use serde::de::DeserializeOwned;

/// The vectors' directory in the core checkout beside this one.
fn vectors() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../shekyl-core/rust/shekyl-rpc-types/tests/vectors/rpc/wire")
}

/// Core's reply vector for `method`, decoded as `T`.
pub(crate) fn reply<T: DeserializeOwned>(method: &str) -> T {
    let path = vectors().join(format!("{method}.reply.json"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("core's {method} reply vector at {}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("core's {method} reply vector does not decode: {e}"))
}
