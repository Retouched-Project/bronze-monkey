// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::engine::registry::DeviceRecord;
use crate::codec::externals::bm_registry_info::{BMRegistryInfo, BMRegistryInfoC};
use crate::codec::messages::bm_encoding::Value;
use std::os::raw::{c_char, c_uchar};
use std::ptr;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum Action {
    Send {
        target_device_id: String,
        channel: i32,
        reliability: i32,
        payload: Vec<u8>,
    },
    UpdateRegistry {
        record: DeviceRecord,
    },
    ChunkSetComplete {
        device_id: String,
        set_id: String,
        blob: Vec<u8>,
    },
    ChunkProgress {
        device_id: String,
        set_id: String,
        current: u32,
        total: u32,
    },
    RegistryEvent {
        kind: RegistryEventKind,
        infos: Vec<BMRegistryInfo>,
        success: Option<bool>,
    },
    Invoke {
        method: String,
        return_method: Option<String>,
        params: Vec<Value>,
        raw_bytes: Vec<u8>,
    },
    ControlConfig {
        touch_enabled: Option<bool>,
        accel_enabled: Option<bool>,
        gyro_enabled: Option<bool>,
        orientation_enabled: Option<bool>,
        touch_interval_ms: Option<i32>,
        accel_interval_ms: Option<i32>,
        gyro_interval_ms: Option<i32>,
        orientation_interval_ms: Option<i32>,
        touch_reliability: Option<i32>,
        control_reliability: Option<i32>,
        control_mode: Option<i32>,
        portal_id: Option<String>,
        return_app_id: Option<String>,
    },
    Handshake {
        current: u32,
        minimum: u32,
    },
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum RegistryEventKind {
    OnRegister,
    OnList,
    OnHostConnected,
    OnHostUpdate,
    OnHostDisconnected,
    DeviceConnectRequested,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum ActionTagC {
    Send = 0,
    UpdateRegistry = 1,
    ChunkSetComplete = 2,
    ChunkProgress = 3,
    RegistryEvent = 5,
    Invoke = 6,
    ControlConfig = 7,
    Handshake = 8,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct ActionListC {
    pub ptr: *mut ActionC,
    pub len: usize,
}

#[repr(C)]
#[derive(Debug)]
pub struct ActionC {
    pub tag: ActionTagC,
    pub channel: i32,
    pub reliability: i32,

    pub payload_ptr: *mut c_uchar,
    pub payload_len: usize,
    pub payload_cap: usize,

    pub device_id_ptr: *mut c_char,
    pub device_id_len: usize,

    pub device_name_ptr: *mut c_char,
    pub device_name_len: usize,

    pub device_type_code: i32,
    pub class_id: i32,

    pub has_address: bool,
    pub addr_ptr: *mut c_char,
    pub addr_len: usize,
    pub addr_unreliable_port: i32,
    pub addr_reliable_port: i32,

    pub registry_kind: i32,
    pub registry_success: i32,
    pub registry_ptr: *mut BMRegistryInfoC,
    pub registry_len: usize,

    pub invoke_method_ptr: *mut c_char,
    pub invoke_method_len: usize,
    pub invoke_return_method_ptr: *mut c_char,
    pub invoke_return_method_len: usize,

    pub chunk_set_id_ptr: *mut c_char,
    pub chunk_set_id_len: usize,
    pub chunk_current: u32,
    pub chunk_total: u32,

    pub control_touch_enabled: i32,
    pub control_accel_enabled: i32,
    pub control_gyro_enabled: i32,
    pub control_orientation_enabled: i32,
    pub control_touch_interval_ms: i32,
    pub control_accel_interval_ms: i32,
    pub control_gyro_interval_ms: i32,
    pub control_orientation_interval_ms: i32,
    pub control_touch_reliability: i32,
    pub control_reliability: i32,
    pub control_mode: i32,
    pub control_portal_id_ptr: *mut c_char,
    pub control_portal_id_len: usize,
    pub control_return_app_id_ptr: *mut c_char,
    pub control_return_app_id_len: usize,
    pub handshake_current: u32,
    pub handshake_minimum: u32,
}

impl Default for ActionC {
    fn default() -> Self {
        Self {
            tag: ActionTagC::Send,
            channel: 0,
            reliability: 0,
            payload_ptr: ptr::null_mut(),
            payload_len: 0,
            payload_cap: 0,
            device_id_ptr: ptr::null_mut(),
            device_id_len: 0,
            device_name_ptr: ptr::null_mut(),
            device_name_len: 0,
            device_type_code: -1,
            class_id: -1,
            has_address: false,
            addr_ptr: ptr::null_mut(),
            addr_len: 0,
            addr_unreliable_port: 0,
            addr_reliable_port: 0,
            registry_kind: -1,
            registry_success: -1,
            registry_ptr: ptr::null_mut(),
            registry_len: 0,
            invoke_method_ptr: ptr::null_mut(),
            invoke_method_len: 0,
            invoke_return_method_ptr: ptr::null_mut(),
            invoke_return_method_len: 0,
            chunk_set_id_ptr: ptr::null_mut(),
            chunk_set_id_len: 0,
            chunk_current: 0,
            chunk_total: 0,
            control_touch_enabled: -1,
            control_accel_enabled: -1,
            control_gyro_enabled: -1,
            control_orientation_enabled: -1,
            control_touch_interval_ms: -1,
            control_accel_interval_ms: -1,
            control_gyro_interval_ms: -1,
            control_orientation_interval_ms: -1,
            control_touch_reliability: -1,
            control_reliability: -1,
            control_mode: -1,
            control_portal_id_ptr: ptr::null_mut(),
            control_portal_id_len: 0,
            control_return_app_id_ptr: ptr::null_mut(),
            control_return_app_id_len: 0,
            handshake_current: 0,
            handshake_minimum: 0,
        }
    }
}

crate::ffi_cstring_accessors!(
    ActionC,
    device_id_ptr,
    device_id_len,
    set_inner = action_set_device_id_inner,
    set = action_set_device_id,
    get_len = action_get_device_id_len,
    get = action_get_device_id,
    free_field = action_free_device_id
);

crate::ffi_cstring_accessors!(
    ActionC,
    device_name_ptr,
    device_name_len,
    set_inner = action_set_device_name_inner,
    set = action_set_device_name,
    get_len = action_get_device_name_len,
    get = action_get_device_name,
    free_field = action_free_device_name
);

crate::ffi_cstring_accessors!(
    ActionC,
    addr_ptr,
    addr_len,
    set_inner = action_set_addr_inner,
    set = action_set_addr,
    get_len = action_get_addr_len,
    get = action_get_addr,
    free_field = action_free_addr
);

crate::ffi_cstring_accessors!(
    ActionC,
    invoke_method_ptr,
    invoke_method_len,
    set_inner = action_set_invoke_method_inner,
    set = action_set_invoke_method,
    get_len = action_get_invoke_method_len,
    get = action_get_invoke_method,
    free_field = action_free_invoke_method
);

crate::ffi_cstring_accessors!(
    ActionC,
    invoke_return_method_ptr,
    invoke_return_method_len,
    set_inner = action_set_invoke_return_method_inner,
    set = action_set_invoke_return_method,
    get_len = action_get_invoke_return_method_len,
    get = action_get_invoke_return_method,
    free_field = action_free_invoke_return_method
);

crate::ffi_cstring_accessors!(
    ActionC,
    chunk_set_id_ptr,
    chunk_set_id_len,
    set_inner = action_set_chunk_set_id_inner,
    set = action_set_chunk_set_id,
    get_len = action_get_chunk_set_id_len,
    get = action_get_chunk_set_id,
    free_field = action_free_chunk_set_id
);

crate::ffi_vec_u8_accessors!(
    ActionC,
    payload_ptr,
    payload_len,
    payload_cap,
    set_inner = action_set_payload_inner,
    set = action_set_payload,
    get_len = action_get_payload_len,
    get = action_get_payload,
    free_field = action_free_payload
);

crate::ffi_cstring_accessors!(
    ActionC,
    control_portal_id_ptr,
    control_portal_id_len,
    set_inner = action_set_control_portal_id_inner,
    set = action_set_control_portal_id,
    get_len = action_get_control_portal_id_len,
    get = action_get_control_portal_id,
    free_field = action_free_control_portal_id
);

crate::ffi_cstring_accessors!(
    ActionC,
    control_return_app_id_ptr,
    control_return_app_id_len,
    set_inner = action_set_control_return_app_id_inner,
    set = action_set_control_return_app_id,
    get_len = action_get_control_return_app_id_len,
    get = action_get_control_return_app_id,
    free_field = action_free_control_return_app_id
);

crate::ffi_free_struct!(
    ActionC,
    action_free,
    action_free_device_id,
    action_free_device_name,
    action_free_addr,
    action_free_invoke_method,
    action_free_invoke_return_method,
    action_free_chunk_set_id,
    action_free_payload,
    action_free_control_portal_id,
    action_free_control_return_app_id
);
