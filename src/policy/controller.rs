// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

//! Controller-role policy: handlers for incoming-from-game messages that
//! benefit from being shared across bindings rather than duplicated at the
//! integration layer.

#[derive(Debug, Clone, Default)]
pub struct ControllerPolicy {}

impl ControllerPolicy {
    pub fn new() -> Self {
        Self::default()
    }
}
