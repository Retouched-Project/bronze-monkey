// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::bm_registry_info::BMRegistryInfo;
use crate::codec::messages::bm_encoding::Value;
use crate::codec::messages::touch::Touch;
use crate::codec::object::Object;
use crate::engine::device_registry::DeviceRecord;
use crate::types::control_mode::ControlMode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct Outgoing {
    pub target_device_id: String,
    pub channel: i32,
    pub reliability: i32,
    pub prefers_datagram: bool,
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct ProcessOutput {
    pub events: Vec<Event>,
    pub outgoings: Vec<Outgoing>,
}

impl ProcessOutput {
    pub fn new() -> Self {
        Self::default()
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[cfg_attr(target_arch = "wasm32", serde(rename_all_fields = "camelCase"))]
pub enum Event {
    Handshake {
        current: u32,
        minimum: u32,
    },
    PeerSeen {
        record: DeviceRecord,
    },
    PeerConnected {
        record: DeviceRecord,
        udp_port: i32,
    },
    ConnectionFailed {
        device_id: String,
    },
    PeerRegistered {
        info: BMRegistryInfo,
        domain: Option<String>,
        success: bool,
    },
    RegistrationResult {
        success: bool,
    },
    SlotAssigned {
        info: BMRegistryInfo,
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
    Relayed {
        sender: Option<String>,
        destination: String,
        method: String,
        return_method: Option<String>,
        params: Vec<Value>,
    },
    DeviceKilled {
        device_id: String,
    },
    Touch {
        sender: String,
        touches: Vec<Touch>,
    },
    Accel {
        sender: String,
        x: f64,
        y: f64,
        z: f64,
    },
    Gyro {
        sender: String,
        x: f32,
        y: f32,
        z: f32,
    },
    Orientation {
        sender: String,
        x: f32,
        y: f32,
        z: f32,
        w: f32,
    },
    DPad {
        sender: String,
        x: i16,
        y: i16,
    },
    Button {
        sender: String,
        handler: String,
        pressed: bool,
    },
    MenuEvent {
        sender: String,
        event: String,
    },
    KeyString {
        sender: String,
        key: String,
    },
    Navigation {
        sender: String,
        nav: String,
    },
    Capabilities {
        sender: String,
        gyroscope: bool,
        orientation: bool,
    },
    ControlConfig(ControlConfig),
    Vibrate {
        sender: String,
    },
    Pause {
        sender: String,
    },
    ControlSchemeRequested {
        sender: String,
        width: i32,
        height: i32,
        requester: String,
    },
    ControlSchemeParsed {
        sender: String,
        device_id: String,
    },
    CookieRequested {
        sender: String,
        name: String,
    },
    CookieStored {
        sender: String,
        name: String,
        value: String,
    },
    Cookie {
        sender: String,
        name: String,
        value: String,
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
        #[serde(with = "serde_bytes")]
        blob: Vec<u8>,
    },
}

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct ControlConfig {
    pub touch_enabled: Option<bool>,
    pub accel_enabled: Option<bool>,
    pub gyro_enabled: Option<bool>,
    pub orientation_enabled: Option<bool>,
    pub touch_interval_ms: Option<i32>,
    pub accel_interval_ms: Option<i32>,
    pub gyro_interval_ms: Option<i32>,
    pub orientation_interval_ms: Option<i32>,
    pub control_mode: Option<ControlMode>,
    pub portal_id: Option<String>,
    pub return_app_id: Option<String>,
    pub start_string: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sensor {
    Touch,
    Accel,
    Gyro,
    Orientation,
}

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[cfg_attr(target_arch = "wasm32", serde(rename_all_fields = "camelCase"))]
pub enum Command {
    Raw {
        target: String,
        channel: i32,
        reliability: i32,
        #[serde(with = "serde_bytes")]
        payload: Vec<u8>,
    },
    SendObject {
        target: String,
        object: Object,
        channel: Option<i32>,
        reliability: Option<i32>,
    },
    Invoke {
        target: String,
        method: String,
        return_method: Option<String>,
        params: Vec<Value>,
    },
    Relay {
        target: String,
        destination: BMRegistryInfo,
        method: String,
        return_method: Option<String>,
        params: Vec<Value>,
    },
    ApproveRegistration {
        device_id: String,
    },
    DenyRegistration {
        device_id: String,
    },
    DropDevice {
        device_id: String,
    },
    Register {
        target: String,
        info: BMRegistryInfo,
        domain: Option<String>,
        return_method: Option<String>,
    },
    RequestHostList {
        target: String,
        return_method: Option<String>,
    },
    UpdateHostInfo {
        target: String,
        info: BMRegistryInfo,
        return_method: Option<String>,
    },
    Unregister {
        target: String,
        return_method: Option<String>,
    },
    SetHostVisible {
        target: String,
        visible: bool,
        notify_everyone: bool,
    },
    ConnectToHost {
        target: String,
        host: BMRegistryInfo,
        self_info: BMRegistryInfo,
    },
    ReportConnectionFailed {
        target: String,
        controller: BMRegistryInfo,
    },
    SendTouch {
        target: String,
        touches: Vec<Touch>,
    },
    SendAccel {
        target: String,
        x: f64,
        y: f64,
        z: f64,
    },
    SendGyro {
        target: String,
        x: f64,
        y: f64,
        z: f64,
    },
    SendOrientation {
        target: String,
        x: f64,
        y: f64,
        z: f64,
        w: f64,
    },
    SendDPad {
        target: String,
        x: i16,
        y: i16,
    },
    SendButton {
        target: String,
        handler: String,
        pressed: bool,
    },
    SendMenuEvent {
        target: String,
        event: String,
    },
    SendKeyString {
        target: String,
        key: String,
    },
    SendNavigation {
        target: String,
        nav: String,
    },
    SetCapabilities {
        target: String,
        gyroscope: bool,
        orientation: bool,
    },
    ConfigureSensor {
        target: String,
        sensor: Sensor,
        enabled: Option<bool>,
        interval_ms: Option<i32>,
    },
    SetReliability {
        target: String,
        touch: i32,
        sensors: i32,
    },
    SetControlMode {
        target: String,
        mode: ControlMode,
        text: Option<String>,
    },
    Vibrate {
        target: String,
    },
    Pause {
        target: String,
    },
    Ping {
        target: String,
    },
    RequestControlScheme {
        target: String,
        width: i32,
        height: i32,
    },
    SendControlScheme {
        target: String,
        #[serde(with = "serde_bytes")]
        xml: Vec<u8>,
    },
    ControlSchemeParsed {
        target: String,
    },
    StoreCookie {
        target: String,
        name: String,
        value: String,
    },
    RequestCookie {
        target: String,
        name: String,
    },
    SendCookie {
        target: String,
        name: String,
        value: String,
    },
    UpdateWallet {
        target: String,
    },
    PromptTrialUpsell {
        target: String,
    },
    WaitForNewHost {
        target: String,
        host_device_id: String,
    },
}
