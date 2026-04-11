// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelType {
    Broadcast = 0,
    Acceleration = 1, // can be unreliable (UDP)
    Touch = 2,        // can be unreliable (UDP)
    Message = 3,
    String = 4,
    Bytes = 5,
    Gyro = 6,        // can be unreliable (UDP)
    Orientation = 7, // can be unreliable (UDP)
    DPad = 8,
}

impl ChannelType {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Broadcast),
            1 => Some(Self::Acceleration),
            2 => Some(Self::Touch),
            3 => Some(Self::Message),
            4 => Some(Self::String),
            5 => Some(Self::Bytes),
            6 => Some(Self::Gyro),
            7 => Some(Self::Orientation),
            8 => Some(Self::DPad),
            _ => None,
        }
    }

    pub fn value(&self) -> i32 {
        *self as i32
    }
}
