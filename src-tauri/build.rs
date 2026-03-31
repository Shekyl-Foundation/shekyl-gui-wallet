// Copyright (c) 2026, The Shekyl Foundation
// BSD-3-Clause license (see LICENSE)

fn main() {
    tauri_build::build();

    // Link against the pre-built Shekyl C++ wallet library.
    // Set SHEKYL_BUILD_DIR to the cmake build directory (e.g., /path/to/Shekyl/build).
    if let Ok(build_dir) = std::env::var("SHEKYL_BUILD_DIR") {
        println!("cargo:rustc-link-search=native={build_dir}/src/wallet");
        println!("cargo:rustc-link-search=native={build_dir}/src/cryptonote_core");
        println!("cargo:rustc-link-search=native={build_dir}/src/cryptonote_basic");
        println!("cargo:rustc-link-search=native={build_dir}/src/mnemonics");
        println!("cargo:rustc-link-search=native={build_dir}/src/common");
        println!("cargo:rustc-link-search=native={build_dir}/src/ringct");
        println!("cargo:rustc-link-search=native={build_dir}/src/device");
        println!("cargo:rustc-link-search=native={build_dir}/src/multisig");
        println!("cargo:rustc-link-search=native={build_dir}/src/net");
        println!("cargo:rustc-link-search=native={build_dir}/src/rpc");
        println!("cargo:rustc-link-search=native={build_dir}/src/serialization");
        println!("cargo:rustc-link-search=native={build_dir}/src/crypto");
        println!("cargo:rustc-link-search=native={build_dir}/contrib/epee/src");
        println!("cargo:rustc-link-search=native={build_dir}/external/easylogging++");
        println!("cargo:rustc-link-search=native={build_dir}/external/db_drivers/liblmdb");

        println!("cargo:rustc-link-lib=static=wallet");
        println!("cargo:rustc-link-lib=static=cryptonote_core");
        println!("cargo:rustc-link-lib=static=cryptonote_basic");
        println!("cargo:rustc-link-lib=static=mnemonics");
        println!("cargo:rustc-link-lib=static=common");
        println!("cargo:rustc-link-lib=static=ringct");
        println!("cargo:rustc-link-lib=static=ringct_basic");
        println!("cargo:rustc-link-lib=static=device");
        println!("cargo:rustc-link-lib=static=multisig");
        println!("cargo:rustc-link-lib=static=net");
        println!("cargo:rustc-link-lib=static=rpc_base");
        println!("cargo:rustc-link-lib=static=serialization");
        println!("cargo:rustc-link-lib=static=cncrypto");
        println!("cargo:rustc-link-lib=static=epee");
        println!("cargo:rustc-link-lib=static=easylogging");
        println!("cargo:rustc-link-lib=static=lmdb");

        println!("cargo:rustc-link-lib=dylib=stdc++");
        println!("cargo:rustc-link-lib=dylib=boost_system");
        println!("cargo:rustc-link-lib=dylib=boost_filesystem");
        println!("cargo:rustc-link-lib=dylib=boost_thread");
        println!("cargo:rustc-link-lib=dylib=boost_serialization");
        println!("cargo:rustc-link-lib=dylib=boost_program_options");
        println!("cargo:rustc-link-lib=dylib=boost_chrono");
        println!("cargo:rustc-link-lib=dylib=boost_date_time");
        println!("cargo:rustc-link-lib=dylib=ssl");
        println!("cargo:rustc-link-lib=dylib=crypto");
        println!("cargo:rustc-link-lib=dylib=sodium");
        println!("cargo:rustc-link-lib=dylib=unbound");
    } else {
        println!("cargo:warning=SHEKYL_BUILD_DIR not set. Wallet FFI functions will not link. Set it to your Shekyl cmake build directory.");
    }
}
