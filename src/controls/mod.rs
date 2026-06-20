// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

pub mod assembler;
pub mod merge;
pub mod parser;

pub const CONTROL_SCHEME_SET_ID: &str = "testXML";
pub const UPDATE_SCHEME_SET_ID: &str = "updateXML";

include!(concat!(env!("OUT_DIR"), "/controls.rs"));
