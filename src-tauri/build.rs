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

    // shekyl-ffi is now a Cargo dependency (rlib), so its #[no_mangle]
    // symbols are compiled into the cdylib without bundling a second libstd.
    // The old approach (rustc-link-lib=static=shekyl_ffi) linked the
    // staticlib archive which bundles libstd, causing duplicate
    // rust_eh_personality in the cdylib output.

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
        "src/net",
        "src/fcmp",
        "src/rpc",
        "src/serialization",
        "contrib/epee/src",
        "external/easylogging++",
        "external/db_drivers/liblmdb",
        "external/randomx",
    ];
    for d in &search_dirs {
        println!("cargo:rustc-link-search=native={build_dir}/{d}");
        if cfg!(target_os = "windows") {
            println!("cargo:rustc-link-search=native={build_dir}/{d}/Release");
        }
    }

    // ── Shekyl static libraries (order matters -- dependents before deps) ──
    let static_libs = [
        "wallet",
        "cryptonote_core",
        "blockchain_db",
        "cryptonote_basic",
        "cryptonote_format_utils_basic",
        "fcmp",
        "fcmp_basic",
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
    let wallet_crypto_candidates = [
        format!("{build_dir}/src/crypto/wallet/libwallet-crypto.a"),
        format!("{build_dir}/src/crypto/wallet/wallet-crypto.lib"),
        format!("{build_dir}/src/crypto/wallet/Release/wallet-crypto.lib"),
    ];
    if wallet_crypto_candidates
        .iter()
        .any(|p| std::path::Path::new(p).exists())
    {
        println!("cargo:rustc-link-lib=static=wallet-crypto");
    }

    // ── Platform-specific system / shared libraries ─────────────────────
    if cfg!(target_os = "linux") {
        link_linux();
    } else if cfg!(target_os = "macos") {
        let homebrew_lib = brew_prefix_lib(None);
        let openssl_lib = brew_prefix_lib(Some("openssl@3"));
        if let Some(ref lib_dir) = homebrew_lib {
            println!("cargo:rustc-link-search=native={lib_dir}");
        }
        if let Some(ref lib_dir) = openssl_lib {
            println!("cargo:rustc-link-search=native={lib_dir}");
        }

        println!("cargo:rustc-link-lib=dylib=c++");

        // Boost libraries -- only link ones that exist as files.
        // Some components (e.g. Boost.System since 1.69) are header-only
        // in newer Boost releases and produce no library artifact.
        let boost_names = [
            "system",
            "filesystem",
            "thread",
            "serialization",
            "program_options",
            "chrono",
            "date_time",
            "regex",
        ];
        if let Some(ref lib_dir) = homebrew_lib {
            for name in &boost_names {
                let dylib = format!("{lib_dir}/libboost_{name}.dylib");
                if std::path::Path::new(&dylib).exists() {
                    println!("cargo:rustc-link-lib=dylib=boost_{name}");
                }
            }
        }

        for lib in &[
            "ssl", "crypto", "sodium", "unbound", "hidapi", "usb-1.0", "protobuf",
        ] {
            println!("cargo:rustc-link-lib=dylib={lib}");
        }
        println!("cargo:rustc-link-lib=framework=Security");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=IOKit");
    } else if cfg!(target_os = "windows") {
        if let Ok(root) = std::env::var("VCPKG_INSTALLATION_ROOT") {
            let vcpkg_lib = format!("{root}/installed/x64-windows-static/lib");
            println!("cargo:rustc-link-search=native={vcpkg_lib}");
        }

        for lib in &[
            "boost_system",
            "boost_filesystem",
            "boost_thread",
            "boost_serialization",
            "boost_program_options",
            "boost_chrono",
            "boost_date_time",
            "boost_regex",
        ] {
            println!("cargo:rustc-link-lib=static={lib}");
        }
        for lib in &["libssl", "libcrypto", "sodium", "libprotobuf"] {
            println!("cargo:rustc-link-lib=static={lib}");
        }
        for lib in &[
            "ws2_32", "bcrypt", "crypt32", "userenv", "ntdll", "iphlpapi", "advapi32", "ole32",
            "shell32",
        ] {
            println!("cargo:rustc-link-lib=dylib={lib}");
        }
    }
}

/// Link third-party libraries on Linux.
///
/// When `SHEKYL_DEPENDS_PREFIX` is set (CI builds via `contrib/depends`),
/// all third-party libraries are statically linked from the depends prefix.
/// Otherwise, fall back to dynamic linking for local development.
fn link_linux() {
    println!("cargo:rustc-link-lib=dylib=stdc++");

    if let Ok(prefix) = std::env::var("SHEKYL_DEPENDS_PREFIX") {
        println!("cargo:rustc-link-search=native={prefix}/lib");

        for lib in &[
            "boost_system",
            "boost_filesystem",
            "boost_thread",
            "boost_serialization",
            "boost_program_options",
            "boost_chrono",
            "boost_date_time",
            "boost_regex",
            "boost_locale",
        ] {
            println!("cargo:rustc-link-lib=static={lib}");
        }

        for lib in &[
            "ssl",
            "crypto",
            "sodium",
            "unbound",
            "hidapi-hidraw",
            "usb-1.0",
            "protobuf-lite",
            "udev",
        ] {
            let static_path = format!("{prefix}/lib/lib{lib}.a");
            if std::path::Path::new(&static_path).exists() {
                println!("cargo:rustc-link-lib=static={lib}");
            }
        }

        // System libraries that are always dynamically linked.
        // Note: libunwind is intentionally excluded. The depends prefix
        // builds a non-PIC libunwind.a whose TLS relocations
        // (R_X86_64_TPOFF32) are incompatible with cdylib output, and
        // lld resolves -lunwind to that .a even under -Bdynamic. The
        // Rust stdlib bundles its own unwind support, and the system's
        // libunwind.so is pulled in transitively via libstdc++.
        for lib in &["dl", "pthread", "rt", "m"] {
            println!("cargo:rustc-link-lib=dylib={lib}");
        }
    } else {
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
    }
}

/// Returns `{homebrew_prefix}/lib` for the given formula (or the global
/// prefix when `formula` is `None`).  Returns `None` when `brew` is not
/// available or the command fails.
// CLIPPY: Only called in the cfg!(target_os = "macos") branch, which is a
// runtime check (not #[cfg]), so the function compiles on all hosts.
#[allow(dead_code)]
fn brew_prefix_lib(formula: Option<&str>) -> Option<String> {
    let mut cmd = std::process::Command::new("brew");
    cmd.arg("--prefix");
    if let Some(f) = formula {
        cmd.arg(f);
    }
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Some(format!("{prefix}/lib"))
}
