// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::bm_registry_info::BMRegistryInfo;
use crate::codec::messages::bm_encoding::Value;
use crate::engine::registry::DeviceRecord;
use crate::types::packet_type::PacketType;
use serde::Serialize;

/// Bytes the integrator must flush to the wire. Outputs from
/// `process_incoming`, `emit`, and `make_*` will all carry these.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Send {
    pub target_device_id: String,
    pub channel: i32,
    pub reliability: i32,
    pub payload: Vec<u8>,
}

/// Notifications emitted by the engine while processing incoming bytes.
/// Integrators may dispatch on these to update UI / game state. Side-effect-
/// free with respect to the engine itself; ignoring an Event is always safe.
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

/// Subset of control configuration the game can request the controller apply.
/// All fields optional; only set fields are interpreted.
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

/// High-level outgoing operations the integrator can request. `emit(Command)`
/// produces `Vec<Send>` bytes ready to flush. Sits alongside the lower-level
/// `make_*` helpers on Engine for cases where the integrator wants to
/// compose protocol packets directly.
#[derive(Debug, Clone)]
pub enum Command {
    /// Send arbitrary bytes to a target with caller-controlled reliability.
    Raw {
        target: String,
        channel: i32,
        reliability: i32,
        payload: Vec<u8>,
    },
    /// Build and send a BM packet with the given message body. Reliability
    /// is inferred from the channel default if not specified.
    Packet {
        target: String,
        channel: i32,
        reliability: Option<i32>,
        packet_type: PacketType,
        message: Option<Vec<u8>>,
    },
    /// Build and send an invoke RPC to a target.
    Invoke {
        target: String,
        method: String,
        return_method: Option<String>,
        params: Vec<Value>,
    },
    /// Drop a registered device from the engine's state. Server-role
    /// integrators use this when a controller disconnects.
    DropDevice {
        device_id: String,
    },
    /// Approve a pending controller registration. Server-role only.
    /// No-op when called on engines without `ServerPolicy` configured.
    ApproveRegistration {
        device_id: String,
    },
    /// Deny a pending controller registration. Server-role only.
    DenyRegistration {
        device_id: String,
    },
}
