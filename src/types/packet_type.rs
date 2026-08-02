// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(into = "i32", try_from = "i32")]
pub enum PacketType {
    #[default]
    Data = 0,
    Ping = 1,
    Ack = 2,
    Echo = 3,
    Analysis = 4,
    KeepAlive = 5,
}

impl PacketType {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Data),
            1 => Some(Self::Ping),
            2 => Some(Self::Ack),
            3 => Some(Self::Echo),
            4 => Some(Self::Analysis),
            5 => Some(Self::KeepAlive),
            _ => None,
        }
    }

    pub fn code(&self) -> i32 {
        *self as i32
    }
}

impl From<PacketType> for i32 {
    fn from(value: PacketType) -> Self {
        value.code()
    }
}

impl TryFrom<i32> for PacketType {
    type Error = PacketTypeError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Self::from_i32(value).ok_or(PacketTypeError::OutOfRange(value))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PacketTypeError {
    OutOfRange(i32),
}

impl std::fmt::Display for PacketTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PacketTypeError::OutOfRange(v) => write!(f, "PacketType out of range: {v}"),
        }
    }
}

impl std::error::Error for PacketTypeError {}
