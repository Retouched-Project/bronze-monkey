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

impl From<Outgoing> for crate::engine::actions::Action {
    fn from(o: Outgoing) -> Self {
        Self::Send {
            target_device_id: o.target_device_id,
            channel: o.channel,
            reliability: o.reliability,
            payload: o.payload,
        }
    }
}

impl From<Event> for crate::engine::actions::Action {
    fn from(e: Event) -> Self {
        use crate::engine::actions::{Action, RegistryEventKind};
        let registry = |kind, infos, success| Action::RegistryEvent {
            kind,
            infos,
            success,
        };
        match e {
            Event::Handshake { current, minimum } => Action::Handshake { current, minimum },
            Event::PeerSeen { record } => Action::UpdateRegistry { record },
            Event::PeerRegistered { info, success } => {
                registry(RegistryEventKind::OnRegister, vec![info], Some(success))
            }
            Event::HostConnected { info } => {
                registry(RegistryEventKind::OnHostConnected, vec![info], None)
            }
            Event::HostUpdated { info } => {
                registry(RegistryEventKind::OnHostUpdate, vec![info], None)
            }
            Event::HostDisconnected { info } => {
                registry(RegistryEventKind::OnHostDisconnected, vec![info], None)
            }
            Event::HostList { infos } => registry(RegistryEventKind::OnList, infos, None),
            Event::DeviceConnectRequested { info } => {
                registry(RegistryEventKind::DeviceConnectRequested, vec![info], None)
            }
            Event::Invoke {
                method,
                return_method,
                params,
                ..
            } => Action::Invoke {
                method,
                return_method,
                params,
                raw_bytes: Vec::new(),
            },
            Event::ChunkProgress {
                device_id,
                set_id,
                current,
                total,
            } => Action::ChunkProgress {
                device_id,
                set_id,
                current,
                total,
            },
            Event::ChunkComplete {
                device_id,
                set_id,
                blob,
            } => Action::ChunkSetComplete {
                device_id,
                set_id,
                blob,
            },
            Event::ControlConfig(cfg) => Action::ControlConfig {
                touch_enabled: cfg.touch_enabled,
                accel_enabled: cfg.accel_enabled,
                gyro_enabled: cfg.gyro_enabled,
                orientation_enabled: cfg.orientation_enabled,
                touch_interval_ms: cfg.touch_interval_ms,
                accel_interval_ms: cfg.accel_interval_ms,
                gyro_interval_ms: cfg.gyro_interval_ms,
                orientation_interval_ms: cfg.orientation_interval_ms,
                touch_reliability: cfg.touch_reliability,
                control_reliability: cfg.control_reliability,
                control_mode: cfg.control_mode,
                portal_id: cfg.portal_id,
                return_app_id: cfg.return_app_id,
            },
        }
    }
}

impl From<ProcessOutput> for Vec<crate::engine::actions::Action> {
    fn from(out: ProcessOutput) -> Self {
        out.events
            .into_iter()
            .map(Into::into)
            .chain(out.outgoings.into_iter().map(Into::into))
            .collect()
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
