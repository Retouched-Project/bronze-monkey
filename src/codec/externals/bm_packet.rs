// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::types::device_type::DeviceType;
use crate::types::packet_type::PacketType;

#[derive(Debug, Default, Clone)]
pub struct BMPacket {
    pub sequence: i32,
    pub channel: i32,
    pub timestamp: f64,
    pub rtt: f64,
    pub packet_type: PacketType,
    pub device_type: DeviceType,
    pub device_name: String,
    pub device_id: String,
    pub message: Option<Vec<u8>>,
    pub address_host: Option<String>,
    pub addr_unreliable_port: i32,
    pub addr_reliable_port: i32,
}
