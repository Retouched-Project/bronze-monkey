// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BMReliability {
    Unreliable = 0,
    Reliable = 1,
}

impl BMReliability {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Unreliable),
            1 => Some(Self::Reliable),
            _ => None,
        }
    }

    pub fn code(&self) -> i32 {
        *self as i32
    }
}
