// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

fn main() {
    let file_descriptors =
        protox::compile(["src/controls/scheme.proto"], ["src/controls/"]).unwrap();
    prost_build::Config::new()
        .compile_fds(file_descriptors)
        .unwrap();
}
