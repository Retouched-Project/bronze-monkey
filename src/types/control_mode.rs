// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use serde::{Deserialize, Serialize};

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlMode {
    Gamepad = 0,
    Keyboard = 1,
    Navigation = 2,
    Wait = 3,
}

impl ControlMode {
    pub fn from_wire(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Gamepad),
            1 => Some(Self::Keyboard),
            2 => Some(Self::Navigation),
            3 => Some(Self::Wait),
            _ => None,
        }
    }

    pub fn to_wire(self) -> i32 {
        self as i32
    }
}
