// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TouchState {
    Began = 1,
    Moved = 2,
    Stationary = 3,
    Ended = 4,
    Cancelled = 5,
}

impl TouchState {
    pub fn from_value(v: i32) -> Option<Self> {
        match v {
            1 => Some(Self::Began),
            2 => Some(Self::Moved),
            3 => Some(Self::Stationary),
            4 => Some(Self::Ended),
            5 => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub fn value(&self) -> i32 {
        *self as i32
    }
}
