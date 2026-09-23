// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::types::coded::crosses_as_its_code;

crosses_as_its_code! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ControlMode {
        Game = 0,
        Text = 1,
        Nav = 2,
        Wait = 3,
    }
}
