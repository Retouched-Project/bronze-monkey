// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

pub mod assembler;
pub mod builder;
pub mod merge;
pub mod parser;
pub mod writer;

pub const CONTROL_SCHEME_SET_ID: &str = "testXML";
pub const UPDATE_SCHEME_SET_ID: &str = "updateXML";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Screen {
    pub width: i32,
    pub height: i32,
}

include!(concat!(env!("OUT_DIR"), "/controls.rs"));
