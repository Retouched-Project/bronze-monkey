// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use std::os::raw::{c_char, c_uchar};
use std::ptr;

use crate::codec::externals::bm_registry_info::{BMRegistryInfoC, bm_registry_info_free};
use crate::codec::messages::bm_encoding::ValueC;

use super::*;

#[repr(C)]
#[derive(Debug)]
pub struct OutgoingC {
    pub channel: i32,
    pub reliability: i32,
    pub target_device_id_ptr: *mut c_char,
    pub target_device_id_len: usize,
    pub payload_ptr: *mut c_uchar,
    pub payload_len: usize,
    pub payload_cap: usize,
}

impl Default for OutgoingC {
    fn default() -> Self {
        Self {
            channel: 0,
            reliability: 0,
            target_device_id_ptr: ptr::null_mut(),
            target_device_id_len: 0,
            payload_ptr: ptr::null_mut(),
            payload_len: 0,
            payload_cap: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct OutgoingListC {
    pub ptr: *mut OutgoingC,
    pub len: usize,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum EventTagC {
    Handshake = 0,
    PeerSeen = 1,
    PeerRegistered = 2,
    HostConnected = 3,
    HostUpdated = 4,
    HostDisconnected = 5,
    HostList = 6,
    DeviceConnectRequested = 7,
    Invoke = 8,
    ChunkProgress = 9,
    ChunkComplete = 10,
    ControlConfig = 11,
    PeerConnected = 12,
    ConnectionFailed = 13,
    Touch = 14,
    Accel = 15,
    Gyro = 16,
    Orientation = 17,
    DPad = 18,
    Button = 19,
    MenuEvent = 20,
    KeyString = 21,
    Navigation = 22,
    Capabilities = 23,
    Vibrate = 24,
    Pause = 25,
    ControlSchemeRequested = 26,
    ControlSchemeParsed = 27,
    CookieRequested = 28,
    CookieStored = 29,
    Cookie = 30,
    RegistrationResult = 31,
    ControlScheme = 32,
    SlotAssigned = 33,
    DeviceKilled = 34,
}

#[repr(C)]
#[derive(Debug)]
pub struct EventC {
    pub tag: EventTagC,

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

    pub registry_success: i32,
    pub registry_ptr: *mut BMRegistryInfoC,
    pub registry_len: usize,

    pub sender_ptr: *mut c_char,
    pub sender_len: usize,
    pub invoke_method_ptr: *mut c_char,
    pub invoke_method_len: usize,
    pub invoke_return_method_ptr: *mut c_char,
    pub invoke_return_method_len: usize,

    pub payload_ptr: *mut c_uchar,
    pub payload_len: usize,
    pub payload_cap: usize,

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

    pub name_ptr: *mut c_char,
    pub name_len: usize,
    pub value_ptr: *mut c_char,
    pub value_len: usize,

    pub touches_ptr: *mut TouchPointC,
    pub touches_len: usize,

    pub sensor_x: f64,
    pub sensor_y: f64,
    pub sensor_z: f64,
    pub sensor_w: f64,
    pub dpad_x: i16,
    pub dpad_y: i16,
    pub pressed: bool,
    pub cap_gyroscope: bool,
    pub cap_orientation: bool,
    pub scheme_width: i32,
    pub scheme_height: i32,
}

impl Default for EventC {
    fn default() -> Self {
        Self {
            tag: EventTagC::Handshake,
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
            registry_success: -1,
            registry_ptr: ptr::null_mut(),
            registry_len: 0,
            sender_ptr: ptr::null_mut(),
            sender_len: 0,
            invoke_method_ptr: ptr::null_mut(),
            invoke_method_len: 0,
            invoke_return_method_ptr: ptr::null_mut(),
            invoke_return_method_len: 0,
            payload_ptr: ptr::null_mut(),
            payload_len: 0,
            payload_cap: 0,
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
            name_ptr: ptr::null_mut(),
            name_len: 0,
            value_ptr: ptr::null_mut(),
            value_len: 0,
            touches_ptr: ptr::null_mut(),
            touches_len: 0,
            sensor_x: 0.0,
            sensor_y: 0.0,
            sensor_z: 0.0,
            sensor_w: 0.0,
            dpad_x: 0,
            dpad_y: 0,
            pressed: false,
            cap_gyroscope: false,
            cap_orientation: false,
            scheme_width: 0,
            scheme_height: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct EventListC {
    pub ptr: *mut EventC,
    pub len: usize,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct ProcessOutputC {
    pub events: EventListC,
    pub outgoings: OutgoingListC,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandTagC {
    Raw = 0,
    Invoke = 1,
    Relay = 2,
    ApproveRegistration = 3,
    DenyRegistration = 4,
    DropDevice = 5,
    Register = 6,
    RequestHostList = 7,
    UpdateHostInfo = 8,
    Unregister = 9,
    SetHostVisible = 10,
    ConnectToHost = 11,
    SendTouch = 12,
    SendAccel = 13,
    SendGyro = 14,
    SendOrientation = 15,
    SendDPad = 16,
    SendButton = 17,
    SendMenuEvent = 18,
    SendKeyString = 19,
    SendNavigation = 20,
    SetCapabilities = 21,
    ConfigureSensor = 22,
    SetReliability = 23,
    SetControlMode = 24,
    Vibrate = 25,
    Pause = 26,
    RequestControlScheme = 27,
    SendControlScheme = 28,
    ControlSchemeParsed = 29,
    StoreCookie = 30,
    RequestCookie = 31,
    SendCookie = 32,
    Ping = 33,
}

impl CommandTagC {
    pub(super) fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            0 => Self::Raw,
            1 => Self::Invoke,
            2 => Self::Relay,
            3 => Self::ApproveRegistration,
            4 => Self::DenyRegistration,
            5 => Self::DropDevice,
            6 => Self::Register,
            7 => Self::RequestHostList,
            8 => Self::UpdateHostInfo,
            9 => Self::Unregister,
            10 => Self::SetHostVisible,
            11 => Self::ConnectToHost,
            12 => Self::SendTouch,
            13 => Self::SendAccel,
            14 => Self::SendGyro,
            15 => Self::SendOrientation,
            16 => Self::SendDPad,
            17 => Self::SendButton,
            18 => Self::SendMenuEvent,
            19 => Self::SendKeyString,
            20 => Self::SendNavigation,
            21 => Self::SetCapabilities,
            22 => Self::ConfigureSensor,
            23 => Self::SetReliability,
            24 => Self::SetControlMode,
            25 => Self::Vibrate,
            26 => Self::Pause,
            27 => Self::RequestControlScheme,
            28 => Self::SendControlScheme,
            29 => Self::ControlSchemeParsed,
            30 => Self::StoreCookie,
            31 => Self::RequestCookie,
            32 => Self::SendCookie,
            33 => Self::Ping,
            _ => return None,
        })
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CommandC {
    pub tag: i32,

    pub target: *const c_char,
    pub device_id: *const c_char,
    pub method: *const c_char,
    pub return_method: *const c_char,
    pub domain: *const c_char,
    pub name: *const c_char,
    pub value: *const c_char,

    pub params_ptr: *const ValueC,
    pub params_len: usize,
    pub info: *const BMRegistryInfoC,
    pub self_info: *const BMRegistryInfoC,
    pub touches_ptr: *const TouchPointC,
    pub touches_len: usize,
    pub payload_ptr: *const c_uchar,
    pub payload_len: usize,

    pub channel: i32,
    pub reliability: i32,
    pub sensor: i32,
    pub enabled: i32,
    pub interval_ms: i32,
    pub touch_reliability: i32,
    pub sensors_reliability: i32,
    pub mode: i32,
    pub width: i32,
    pub height: i32,

    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
    pub dpad_x: i16,
    pub dpad_y: i16,

    pub pressed: bool,
    pub visible: bool,
    pub notify_everyone: bool,
    pub gyroscope: bool,
    pub orientation: bool,
}

impl Default for CommandC {
    fn default() -> Self {
        Self {
            tag: 0,
            target: ptr::null(),
            device_id: ptr::null(),
            method: ptr::null(),
            return_method: ptr::null(),
            domain: ptr::null(),
            name: ptr::null(),
            value: ptr::null(),
            params_ptr: ptr::null(),
            params_len: 0,
            info: ptr::null(),
            self_info: ptr::null(),
            touches_ptr: ptr::null(),
            touches_len: 0,
            payload_ptr: ptr::null(),
            payload_len: 0,
            channel: 0,
            reliability: 0,
            sensor: 0,
            enabled: -1,
            interval_ms: -1,
            touch_reliability: 0,
            sensors_reliability: 0,
            mode: 0,
            width: 0,
            height: 0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
            dpad_x: 0,
            dpad_y: 0,
            pressed: false,
            visible: false,
            notify_everyone: false,
            gyroscope: false,
            orientation: false,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_command_init(command: *mut CommandC) {
    catch_void(|| {
        if command.is_null() {
            return;
        }
        unsafe {
            *command = CommandC::default();
        }
    });
}

crate::ffi_cstring_accessors!(
    OutgoingC,
    target_device_id_ptr,
    target_device_id_len,
    set_inner = outgoing_set_target_device_id_inner,
    set = outgoing_set_target_device_id,
    get_len = outgoing_get_target_device_id_len,
    get = outgoing_get_target_device_id,
    free_field = outgoing_free_target_device_id
);

crate::ffi_vec_u8_accessors!(
    OutgoingC,
    payload_ptr,
    payload_len,
    payload_cap,
    set_inner = outgoing_set_payload_inner,
    set = outgoing_set_payload,
    get_len = outgoing_get_payload_len,
    get = outgoing_get_payload,
    free_field = outgoing_free_payload
);

crate::ffi_free_struct!(
    OutgoingC,
    outgoing_free,
    outgoing_free_target_device_id,
    outgoing_free_payload
);

crate::ffi_cstring_accessors!(
    EventC,
    device_id_ptr,
    device_id_len,
    set_inner = event_set_device_id_inner,
    set = event_set_device_id,
    get_len = event_get_device_id_len,
    get = event_get_device_id,
    free_field = event_free_device_id
);

crate::ffi_cstring_accessors!(
    EventC,
    device_name_ptr,
    device_name_len,
    set_inner = event_set_device_name_inner,
    set = event_set_device_name,
    get_len = event_get_device_name_len,
    get = event_get_device_name,
    free_field = event_free_device_name
);

crate::ffi_cstring_accessors!(
    EventC,
    addr_ptr,
    addr_len,
    set_inner = event_set_addr_inner,
    set = event_set_addr,
    get_len = event_get_addr_len,
    get = event_get_addr,
    free_field = event_free_addr
);

crate::ffi_cstring_accessors!(
    EventC,
    sender_ptr,
    sender_len,
    set_inner = event_set_sender_inner,
    set = event_set_sender,
    get_len = event_get_sender_len,
    get = event_get_sender,
    free_field = event_free_sender
);

crate::ffi_cstring_accessors!(
    EventC,
    invoke_method_ptr,
    invoke_method_len,
    set_inner = event_set_invoke_method_inner,
    set = event_set_invoke_method,
    get_len = event_get_invoke_method_len,
    get = event_get_invoke_method,
    free_field = event_free_invoke_method
);

crate::ffi_cstring_accessors!(
    EventC,
    invoke_return_method_ptr,
    invoke_return_method_len,
    set_inner = event_set_invoke_return_method_inner,
    set = event_set_invoke_return_method,
    get_len = event_get_invoke_return_method_len,
    get = event_get_invoke_return_method,
    free_field = event_free_invoke_return_method
);

crate::ffi_cstring_accessors!(
    EventC,
    chunk_set_id_ptr,
    chunk_set_id_len,
    set_inner = event_set_chunk_set_id_inner,
    set = event_set_chunk_set_id,
    get_len = event_get_chunk_set_id_len,
    get = event_get_chunk_set_id,
    free_field = event_free_chunk_set_id
);

crate::ffi_cstring_accessors!(
    EventC,
    control_portal_id_ptr,
    control_portal_id_len,
    set_inner = event_set_control_portal_id_inner,
    set = event_set_control_portal_id,
    get_len = event_get_control_portal_id_len,
    get = event_get_control_portal_id,
    free_field = event_free_control_portal_id
);

crate::ffi_cstring_accessors!(
    EventC,
    control_return_app_id_ptr,
    control_return_app_id_len,
    set_inner = event_set_control_return_app_id_inner,
    set = event_set_control_return_app_id,
    get_len = event_get_control_return_app_id_len,
    get = event_get_control_return_app_id,
    free_field = event_free_control_return_app_id
);

crate::ffi_cstring_accessors!(
    EventC,
    name_ptr,
    name_len,
    set_inner = event_set_name_inner,
    set = event_set_name,
    get_len = event_get_name_len,
    get = event_get_name,
    free_field = event_free_name
);

crate::ffi_cstring_accessors!(
    EventC,
    value_ptr,
    value_len,
    set_inner = event_set_value_inner,
    set = event_set_value,
    get_len = event_get_value_len,
    get = event_get_value,
    free_field = event_free_value
);

crate::ffi_vec_u8_accessors!(
    EventC,
    payload_ptr,
    payload_len,
    payload_cap,
    set_inner = event_set_payload_inner,
    set = event_set_payload,
    get_len = event_get_payload_len,
    get = event_get_payload,
    free_field = event_free_payload
);

crate::ffi_free_struct!(
    EventC,
    event_free,
    event_free_device_id,
    event_free_device_name,
    event_free_addr,
    event_free_sender,
    event_free_invoke_method,
    event_free_invoke_return_method,
    event_free_chunk_set_id,
    event_free_control_portal_id,
    event_free_control_return_app_id,
    event_free_name,
    event_free_value,
    event_free_payload
);

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_outgoings_free(list: *mut OutgoingListC) {
    catch_void(|| {
        if list.is_null() {
            return;
        }
        free_outgoing_list(unsafe { &mut *list });
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_events_free(list: *mut EventListC) {
    catch_void(|| {
        if list.is_null() {
            return;
        }
        free_event_list(unsafe { &mut *list });
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_process_output_free(out: *mut ProcessOutputC) {
    catch_void(|| {
        if out.is_null() {
            return;
        }
        let out_ref = unsafe { &mut *out };
        free_event_list(&mut out_ref.events);
        free_outgoing_list(&mut out_ref.outgoings);
    });
}

fn free_outgoing_list(list: &mut OutgoingListC) {
    if list.ptr.is_null() || list.len == 0 {
        list.ptr = ptr::null_mut();
        list.len = 0;
        return;
    }
    let slice = unsafe { std::slice::from_raw_parts_mut(list.ptr, list.len) };
    for o in slice.iter_mut() {
        outgoing_free(o);
    }
    unsafe {
        let _ = Box::from_raw(slice);
    }
    list.ptr = ptr::null_mut();
    list.len = 0;
}

fn free_event_list(list: &mut EventListC) {
    if list.ptr.is_null() || list.len == 0 {
        list.ptr = ptr::null_mut();
        list.len = 0;
        return;
    }
    let slice = unsafe { std::slice::from_raw_parts_mut(list.ptr, list.len) };
    for e in slice.iter_mut() {
        event_free(e);
        if !e.registry_ptr.is_null() && e.registry_len > 0 {
            unsafe {
                let r_slice = std::slice::from_raw_parts_mut(e.registry_ptr, e.registry_len);
                for r in r_slice.iter_mut() {
                    bm_registry_info_free(r);
                }
                let _ = Box::from_raw(r_slice);
            }
            e.registry_ptr = ptr::null_mut();
            e.registry_len = 0;
        }
        if !e.touches_ptr.is_null() && e.touches_len > 0 {
            unsafe {
                let t_slice = std::slice::from_raw_parts_mut(e.touches_ptr, e.touches_len);
                let _ = Box::from_raw(t_slice);
            }
            e.touches_ptr = ptr::null_mut();
            e.touches_len = 0;
        }
    }
    unsafe {
        let _ = Box::from_raw(slice);
    }
    list.ptr = ptr::null_mut();
    list.len = 0;
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TouchPointC {
    pub id: i32,
    pub x: f32,
    pub y: f32,
    pub screen_width: i16,
    pub screen_height: i16,
    pub state: i32,
}
