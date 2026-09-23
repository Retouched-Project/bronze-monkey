// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::types::coded::crosses_as_its_code;

crosses_as_its_code! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TouchState {
        Began = 1,
        Moved = 2,
        Stationary = 3,
        Ended = 4,
        Cancelled = 5,
    }
}
