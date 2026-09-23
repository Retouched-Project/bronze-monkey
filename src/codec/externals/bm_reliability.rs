// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::types::coded::crosses_as_its_code;

crosses_as_its_code! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum BMReliability {
        Unreliable = 0,
        ReliableUnordered = 1,
        ReliableOrdered = 2,
    }
}
