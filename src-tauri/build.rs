// Copyright (c) 2026, The Shekyl Foundation
// BSD-3-Clause license (see LICENSE)

fn main() {
    tauri_build::build();

    link_shekyl_ffi();
}

fn link_shekyl_ffi() {
    let build_dir = match std::env::var("SHEKYL_BUILD_DIR") {
        Ok(d) => d,
        Err(_) => {
            println!("cargo:warning=SHEKYL_BUILD_DIR not set. Wallet FFI functions will not link. Set it to your Shekyl cmake build directory.");
            return;
        }
    };

    // Derive source dir for Rust FFI library (rust/target/<profile>/).
    // Default: SHEKYL_BUILD_DIR/../  (i.e., the repo root).
    let source_dir = std::env::var("SHEKYL_SOURCE_DIR").unwrap_or_else(|_| {
        std::path::Path::new(&build_dir)
            .parent()
            .unwrap_or(std::path::Path::new(&build_dir))
            .to_string_lossy()
            .into_owned()
    });

    // Link the Rust shekyl-ffi crate (economics, PQC, memory ops, etc.).
    // CMake builds it via BuildRust.cmake into rust/target/[<triple>/]<profile>/.
    let rust_triples = [
        "",
        "x86_64-unknown-linux-gnu",
        "aarch64-apple-darwin",
        "x86_64-apple-darwin",
        "x86_64-pc-windows-msvc",
        "x86_64-pc-windows-gnu",
    ];
    for triple in &rust_triples {
        for profile in ["release", "debug"] {
            if triple.is_empty() {
                println!("cargo:rustc-link-search=native={source_dir}/rust/target/{profile}");
            } else {
                println!(
                    "cargo:rustc-link-search=native={source_dir}/rust/target/{triple}/{profile}"
                );
            }
        }
    }
    println!("cargo:rustc-link-lib=static=shekyl_ffi");

    // ── Search paths ────────────────────────────────────────────────────
    let search_dirs = [
        "lib",
        "src",
        "src/blockchain_db",
        "src/checkpoints",
        "src/common",
        "src/crypto",
        "src/crypto/wallet",
        "src/cryptonote_basic",
        "src/cryptonote_core",
        "src/device",
        "src/device_trezor",
        "src/hardforks",
        "src/mnemonics",
        "src/multisig",
        "src/net",
        "src/ringct",
        "src/rpc",
        "src/serialization",
        "contrib/epee/src",
        "external/easylogging++",
        "external/db_drivers/liblmdb",
        "external/randomx",
    ];
    for d in &search_dirs {
        println!("cargo:rustc-link-search=native={build_dir}/{d}");
    }

    // ── Shekyl static libraries (order matters -- dependents before deps) ──
    let static_libs = [
        "wallet",
        "cryptonote_core",
        "blockchain_db",
        "cryptonote_basic",
        "cryptonote_format_utils_basic",
        "ringct",
        "ringct_basic",
        "multisig",
        "common",
        "device",
        "device_trezor",
        "net",
        "rpc_base",
        "checkpoints",
        "hardforks",
        "serialization",
        "mnemonics",
        "cncrypto",
        "randomx",
        "epee",
        "easylogging",
        "lmdb",
        "version",
    ];
    for lib in &static_libs {
        println!("cargo:rustc-link-lib=static={lib}");
    }

    // wallet-crypto is a separate library only when optimized crypto is available
    // (e.g., x86_64 with amd64 extensions). On platforms where crypto autodetect
    // fails (macOS arm64), CMake aliases it to cncrypto, so no .a file exists.
    let wallet_crypto_path = format!("{build_dir}/src/crypto/wallet/libwallet-crypto.a");
    if std::path::Path::new(&wallet_crypto_path).exists() {
        println!("cargo:rustc-link-lib=static=wallet-crypto");
    }

    // ── Platform-specific system / shared libraries ─────────────────────
    if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-lib=dylib=stdc++");
        for lib in &[
            "boost_system",
            "boost_filesystem",
            "boost_thread",
            "boost_serialization",
            "boost_program_options",
            "boost_chrono",
            "boost_date_time",
            "boost_regex",
            "ssl",
            "crypto",
            "sodium",
            "unbound",
            "hidapi-hidraw",
            "usb-1.0",
            "protobuf",
            "udev",
        ] {
            println!("cargo:rustc-link-lib=dylib={lib}");
        }
    } else if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=dylib=c++");
        for lib in &[
            "boost_system",
            "boost_filesystem",
            "boost_thread",
            "boost_serialization",
            "boost_program_options",
            "boost_chrono",
            "boost_date_time",
            "boost_regex",
            "ssl",
            "crypto",
            "sodium",
            "unbound",
            "hidapi",
            "usb-1.0",
            "protobuf",
        ] {
            println!("cargo:rustc-link-lib=dylib={lib}");
        }
        println!("cargo:rustc-link-lib=framework=Security");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=IOKit");
    } else if cfg!(target_os = "windows") {
        for lib in &[
            "ws2_32", "bcrypt", "crypt32", "userenv", "ntdll", "iphlpapi",
        ] {
            println!("cargo:rustc-link-lib=dylib={lib}");
        }
    }
}
