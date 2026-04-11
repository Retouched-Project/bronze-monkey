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
    pub reliability: i32,

    pub device_name: String,
    pub device_id: String,

    pub message: Option<Vec<u8>>,

    pub address_host: Option<String>,
    pub addr_unreliable_port: i32,
    pub addr_reliable_port: i32,
}

impl BMPacket {
    pub fn new(
        sequence: i32,
        channel: i32,
        timestamp: f64,
        rtt: f64,
        packet_type: PacketType,
        device_type: DeviceType,
        reliability: i32,
        device_name: String,
        device_id: String,
        message: Option<Vec<u8>>,
        address_host: Option<String>,
        addr_unreliable_port: i32,
        addr_reliable_port: i32,
    ) -> Self {
        Self {
            sequence,
            channel,
            timestamp,
            rtt,
            packet_type,
            device_type,
            reliability,
            device_name,
            device_id,
            message,
            address_host,
            addr_unreliable_port,
            addr_reliable_port,
        }
    }
}
