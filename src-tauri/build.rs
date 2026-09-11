// Copyright (c) 2026, The Shekyl Foundation
// BSD-3-Clause license (see LICENSE)

fn main() {
    // The wallet runs entirely on the pure-Rust Engine backend; the C++
    // wallet2 static-library linkage (libwallet.a, cryptonote_core, boost,
    // randomx, …) was removed with the Wallet2 path, so no `SHEKYL_BUILD_DIR`
    // / cmake artifacts are required to link the app.
    tauri_build::build();
}
