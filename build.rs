// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

fn main() {
    let file_descriptors =
        protox::compile(["src/controls/scheme.proto"], ["src/controls/"]).unwrap();
    prost_build::Config::new()
        .compile_fds(file_descriptors)
        .unwrap();

    #[cfg(feature = "cbindgen")]
    generate_c_header();
}

#[cfg(feature = "cbindgen")]
fn generate_c_header() {
    unsafe {
        std::env::set_var("RUSTC_BOOTSTRAP", "1");
    }
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let config =
        cbindgen::Config::from_file(std::path::Path::new(&crate_dir).join("cbindgen.toml"))
            .unwrap_or_default();
    match cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .generate()
    {
        Ok(bindings) => {
            bindings.write_to_file(std::path::Path::new(&crate_dir).join("bronze_monkey.h"));
        }
        Err(e) => println!("cargo:warning=cbindgen header generation failed: {e}"),
    }
}
