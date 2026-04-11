// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
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
