// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::bm_registry_info::BMRegistryInfo;
use crate::codec::messages::bm_encoding::Value;
use crate::engine::registry::DeviceRecord;
use crate::types::packet_type::PacketType;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outgoing {
    pub target_device_id: String,
    pub channel: i32,
    pub reliability: i32,
    pub payload: Vec<u8>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessOutput {
    pub events: Vec<Event>,
    pub outgoings: Vec<Outgoing>,
}

impl ProcessOutput {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum Event {
    Handshake {
        current: u32,
        minimum: u32,
    },
    PeerSeen {
        record: DeviceRecord,
    },
    PeerRegistered {
        info: BMRegistryInfo,
        success: bool,
    },
    HostConnected {
        info: BMRegistryInfo,
    },
    HostUpdated {
        info: BMRegistryInfo,
    },
    HostDisconnected {
        info: BMRegistryInfo,
    },
    HostList {
        infos: Vec<BMRegistryInfo>,
    },
    DeviceConnectRequested {
        info: BMRegistryInfo,
    },
    Invoke {
        sender: Option<String>,
        method: String,
        return_method: Option<String>,
        params: Vec<Value>,
    },
    ChunkProgress {
        device_id: String,
        set_id: String,
        current: u32,
        total: u32,
    },
    ChunkComplete {
        device_id: String,
        set_id: String,
        blob: Vec<u8>,
    },
    ControlConfig(ControlConfig),
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlConfig {
    pub touch_enabled: Option<bool>,
    pub accel_enabled: Option<bool>,
    pub gyro_enabled: Option<bool>,
    pub orientation_enabled: Option<bool>,
    pub touch_interval_ms: Option<i32>,
    pub accel_interval_ms: Option<i32>,
    pub gyro_interval_ms: Option<i32>,
    pub orientation_interval_ms: Option<i32>,
    pub touch_reliability: Option<i32>,
    pub control_reliability: Option<i32>,
    pub control_mode: Option<i32>,
    pub portal_id: Option<String>,
    pub return_app_id: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Command {
    Raw {
        target: String,
        channel: i32,
        reliability: i32,
        payload: Vec<u8>,
    },
    Packet {
        target: String,
        channel: i32,
        reliability: Option<i32>,
        packet_type: PacketType,
        message: Option<Vec<u8>>,
    },
    Invoke {
        target: String,
        method: String,
        return_method: Option<String>,
        params: Vec<Value>,
    },
    DropDevice {
        device_id: String,
    },
    ApproveRegistration {
        device_id: String,
    },
    DenyRegistration {
        device_id: String,
    },
}
