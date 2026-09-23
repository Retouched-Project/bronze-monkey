// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::types::coded::crosses_as_its_code;

crosses_as_its_code! {
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
}

impl PacketType {
    pub fn label(&self) -> &'static str {
        match self {
            PacketType::Data => "DATA",
            PacketType::Ping => "PING",
            PacketType::Ack => "ACK",
            PacketType::Echo => "ECHO",
            PacketType::Analysis => "ANALYSIS",
            PacketType::KeepAlive => "KEEP_ALIVE",
        }
    }
}
