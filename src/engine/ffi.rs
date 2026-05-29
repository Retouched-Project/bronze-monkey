// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use prost::Message;
use std::os::raw::{c_char, c_uchar};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use crate::codec::externals::bm_registry_info::{
    BMRegistryInfoC, bm_registry_info_free, bm_registry_info_set_addr_inner,
    bm_registry_info_set_app_id_inner, bm_registry_info_set_device_id_inner,
    bm_registry_info_set_device_name_inner,
};
use crate::codec::messages::bm_encoding::Value;
use crate::codec::messages::bm_invoke::{BMInvoke, BMInvokeC};
use crate::codec::messages::bm_parameter::VecOutput;
use crate::codec::messages::touch::Touch;
use crate::codec::object::Object;
use crate::controls::parser::BMApplicationSchemeParser;
use crate::devices::device_core::DeviceCoreC;
use crate::engine::events::{Event, Outgoing, ProcessOutput};
use crate::engine::processing::Engine;
use crate::engine::registry::DeviceRecord;
use crate::types::touch_state::TouchState;

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

    pub invoke_sender_ptr: *mut c_char,
    pub invoke_sender_len: usize,
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
            invoke_sender_ptr: ptr::null_mut(),
            invoke_sender_len: 0,
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
    invoke_sender_ptr,
    invoke_sender_len,
    set_inner = event_set_invoke_sender_inner,
    set = event_set_invoke_sender,
    get_len = event_get_invoke_sender_len,
    get = event_get_invoke_sender,
    free_field = event_free_invoke_sender
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
    event_free_invoke_sender,
    event_free_invoke_method,
    event_free_invoke_return_method,
    event_free_chunk_set_id,
    event_free_control_portal_id,
    event_free_control_return_app_id,
    event_free_payload
);

#[inline]
fn catch_bool<F: FnOnce() -> bool>(f: F) -> bool {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(false)
}

#[inline]
fn catch_ptr<T, F: FnOnce() -> *mut T>(f: F) -> *mut T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(std::ptr::null_mut())
}

#[inline]
fn catch_void<F: FnOnce()>(f: F) {
    let _ = catch_unwind(AssertUnwindSafe(f));
}

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
    }
    unsafe {
        let _ = Box::from_raw(slice);
    }
    list.ptr = ptr::null_mut();
    list.len = 0;
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_new() -> *mut Engine {
    catch_ptr(|| Box::into_raw(Box::new(Engine::new())))
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_free(ptr_engine: *mut Engine) {
    catch_void(|| {
        if ptr_engine.is_null() {
            return;
        }
        unsafe {
            drop(Box::from_raw(ptr_engine));
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_set_auto_approve_registration(
    ptr_engine: *mut Engine,
    value: bool,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        engine.server_policy.auto_approve_registration = value;
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_approve_registration(
    ptr_engine: *mut Engine,
    device_id_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() || device_id_ptr.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = {
            let c_str = unsafe { std::ffi::CStr::from_ptr(device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.approve_registration(&dev_id);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_deny_registration(
    ptr_engine: *mut Engine,
    device_id_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() || device_id_ptr.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = {
            let c_str = unsafe { std::ffi::CStr::from_ptr(device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.deny_registration(&dev_id);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_init_local_device(
    ptr_engine: *mut Engine,
    device_core: *const DeviceCoreC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || device_core.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let core_c = unsafe { &*device_core };
        if let Some(core) = core_c.to_rust() {
            engine.init_local_device(core);
            true
        } else {
            false
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_register_device(
    ptr_engine: *mut Engine,
    device_core: *const DeviceCoreC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || device_core.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let core_c = unsafe { &*device_core };
        if let Some(core) = core_c.to_rust() {
            let record = DeviceRecord::new(core, None, None);
            engine.registry_mut().upsert(record);
            true
        } else {
            false
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_process_incoming(
    ptr_engine: *mut Engine,
    payload: *const c_uchar,
    payload_len: usize,
    out: *mut ProcessOutputC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out.is_null() {
            return false;
        }
        if payload_len > 0 && payload.is_null() {
            return false;
        }

        let engine = unsafe { &mut *ptr_engine };
        let bytes = unsafe { std::slice::from_raw_parts(payload, payload_len) };
        let result = process_output_to_c(engine.process_incoming(bytes));
        unsafe {
            *out = result;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_process_incoming_udp(
    ptr_engine: *mut Engine,
    payload: *const c_uchar,
    payload_len: usize,
    out: *mut ProcessOutputC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out.is_null() {
            return false;
        }
        if payload_len > 0 && payload.is_null() {
            return false;
        }

        let engine = unsafe { &mut *ptr_engine };
        let bytes = unsafe { std::slice::from_raw_parts(payload, payload_len) };
        let result = process_output_to_c(engine.process_incoming_udp(bytes));
        unsafe {
            *out = result;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_packet(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    channel: i32,
    reliability: i32,
    packet_type_code: i32,
    message_ptr: *const c_uchar,
    message_len: usize,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        if message_len > 0 && message_ptr.is_null() {
            return false;
        }

        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };

        let msg = if message_len == 0 {
            None
        } else {
            let bytes = unsafe { std::slice::from_raw_parts(message_ptr, message_len) };
            Some(bytes.to_vec())
        };

        let rel = if reliability < 0 {
            None
        } else {
            Some(reliability)
        };
        let packet_type = match crate::types::packet_type::PacketType::from_i32(packet_type_code) {
            Some(pt) => pt,
            None => return false,
        };
        let actions = engine.make_packet(&dev_id, channel, rel, packet_type, msg);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[inline]
fn set_string_field<F>(s: String, setter: F)
where
    F: FnOnce(*const c_char) -> bool,
{
    let c = std::ffi::CString::new(s).unwrap();
    setter(c.as_ptr());
}

fn encode_invoke_message(method: &str, return_method: Option<&str>, params: Vec<Value>) -> Vec<u8> {
    let inv = BMInvoke {
        id: 0,
        method: method.to_string(),
        return_method: return_method.map(|s| s.to_string()),
        params,
    };
    let mut out = VecOutput::default();
    let _ = Object::BMInvoke(inv).encode_with_marker(&mut out);
    out.buf
}

fn set_event_registry(
    out: &mut EventC,
    infos: Vec<crate::codec::externals::bm_registry_info::BMRegistryInfo>,
) {
    let regs: Vec<BMRegistryInfoC> = infos.into_iter().map(registry_info_to_c).collect();
    let len = regs.len();
    let mut boxed = regs.into_boxed_slice();
    out.registry_ptr = boxed.as_mut_ptr();
    out.registry_len = len;
    std::mem::forget(boxed);
}

fn outgoing_to_c(o: Outgoing) -> OutgoingC {
    let mut out = OutgoingC::default();
    out.channel = o.channel;
    out.reliability = o.reliability;
    outgoing_set_payload_inner(&mut out, o.payload.as_ptr(), o.payload.len());
    set_string_field(o.target_device_id, |p| {
        outgoing_set_target_device_id_inner(&mut out, p)
    });
    out
}

fn outgoings_to_c(outgoings: Vec<Outgoing>) -> OutgoingListC {
    let converted: Vec<OutgoingC> = outgoings.into_iter().map(outgoing_to_c).collect();
    let len = converted.len();
    let mut boxed = converted.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    OutgoingListC { ptr, len }
}

fn event_to_c(event: Event) -> EventC {
    let mut out = EventC::default();
    match event {
        Event::Handshake { current, minimum } => {
            out.tag = EventTagC::Handshake;
            out.handshake_current = current;
            out.handshake_minimum = minimum;
        }
        Event::PeerSeen { record } => {
            out.tag = EventTagC::PeerSeen;
            set_string_field(record.core.device_id.clone(), |p| {
                event_set_device_id_inner(&mut out, p)
            });
            set_string_field(record.core.device_name.clone(), |p| {
                event_set_device_name_inner(&mut out, p)
            });
            out.device_type_code = record.core.device_type.code();
            out.class_id = record.class_id.map(|v| v as i32).unwrap_or(-1);
            if let Some(addr) = &record.core.address {
                set_string_field(addr.address.clone(), |p| event_set_addr_inner(&mut out, p));
                out.has_address = true;
                out.addr_unreliable_port = addr.unreliable_port;
                out.addr_reliable_port = addr.reliable_port;
            }
        }
        Event::PeerRegistered { info, success } => {
            out.tag = EventTagC::PeerRegistered;
            out.registry_success = if success { 1 } else { 0 };
            set_event_registry(&mut out, vec![info]);
        }
        Event::HostConnected { info } => {
            out.tag = EventTagC::HostConnected;
            set_event_registry(&mut out, vec![info]);
        }
        Event::HostUpdated { info } => {
            out.tag = EventTagC::HostUpdated;
            set_event_registry(&mut out, vec![info]);
        }
        Event::HostDisconnected { info } => {
            out.tag = EventTagC::HostDisconnected;
            set_event_registry(&mut out, vec![info]);
        }
        Event::HostList { infos } => {
            out.tag = EventTagC::HostList;
            set_event_registry(&mut out, infos);
        }
        Event::DeviceConnectRequested { info } => {
            out.tag = EventTagC::DeviceConnectRequested;
            set_event_registry(&mut out, vec![info]);
        }
        Event::Invoke {
            sender,
            method,
            return_method,
            params,
        } => {
            out.tag = EventTagC::Invoke;
            if let Some(s) = sender {
                set_string_field(s, |p| event_set_invoke_sender_inner(&mut out, p));
            }
            let msg = encode_invoke_message(&method, return_method.as_deref(), params);
            set_string_field(method, |p| event_set_invoke_method_inner(&mut out, p));
            if let Some(rm) = return_method {
                set_string_field(rm, |p| event_set_invoke_return_method_inner(&mut out, p));
            }
            event_set_payload_inner(&mut out, msg.as_ptr(), msg.len());
        }
        Event::ChunkProgress {
            device_id,
            set_id,
            current,
            total,
        } => {
            out.tag = EventTagC::ChunkProgress;
            set_string_field(device_id, |p| event_set_device_id_inner(&mut out, p));
            set_string_field(set_id, |p| event_set_chunk_set_id_inner(&mut out, p));
            out.chunk_current = current;
            out.chunk_total = total;
        }
        Event::ChunkComplete {
            device_id,
            set_id,
            blob,
        } => {
            out.tag = EventTagC::ChunkComplete;
            set_string_field(device_id, |p| event_set_device_id_inner(&mut out, p));
            set_string_field(set_id, |p| event_set_chunk_set_id_inner(&mut out, p));
            event_set_payload_inner(&mut out, blob.as_ptr(), blob.len());
        }
        Event::ControlConfig(cfg) => {
            out.tag = EventTagC::ControlConfig;
            out.control_touch_enabled = cfg
                .touch_enabled
                .map(|v| if v { 1 } else { 0 })
                .unwrap_or(-1);
            out.control_accel_enabled = cfg
                .accel_enabled
                .map(|v| if v { 1 } else { 0 })
                .unwrap_or(-1);
            out.control_gyro_enabled = cfg
                .gyro_enabled
                .map(|v| if v { 1 } else { 0 })
                .unwrap_or(-1);
            out.control_orientation_enabled = cfg
                .orientation_enabled
                .map(|v| if v { 1 } else { 0 })
                .unwrap_or(-1);
            out.control_touch_interval_ms = cfg.touch_interval_ms.unwrap_or(-1);
            out.control_accel_interval_ms = cfg.accel_interval_ms.unwrap_or(-1);
            out.control_gyro_interval_ms = cfg.gyro_interval_ms.unwrap_or(-1);
            out.control_orientation_interval_ms = cfg.orientation_interval_ms.unwrap_or(-1);
            out.control_touch_reliability = cfg.touch_reliability.unwrap_or(-1);
            out.control_reliability = cfg.control_reliability.unwrap_or(-1);
            out.control_mode = cfg.control_mode.unwrap_or(-1);
            if let Some(p) = cfg.portal_id {
                set_string_field(p, |ptr| event_set_control_portal_id_inner(&mut out, ptr));
            }
            if let Some(r) = cfg.return_app_id {
                set_string_field(r, |ptr| {
                    event_set_control_return_app_id_inner(&mut out, ptr)
                });
            }
        }
    }
    out
}

fn events_to_c(events: Vec<Event>) -> EventListC {
    let converted: Vec<EventC> = events.into_iter().map(event_to_c).collect();
    let len = converted.len();
    let mut boxed = converted.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    EventListC { ptr, len }
}

fn process_output_to_c(out: ProcessOutput) -> ProcessOutputC {
    ProcessOutputC {
        events: events_to_c(out.events),
        outgoings: outgoings_to_c(out.outgoings),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_drop_device(
    ptr_engine: *mut Engine,
    device_id_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() || device_id_ptr.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = {
            let c_str = unsafe { std::ffi::CStr::from_ptr(device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.drop_device(&dev_id);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_get_registry(
    ptr_engine: *mut Engine,
    out_ptr: *mut *mut BMRegistryInfoC,
    out_len: *mut usize,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let records = engine.registry().snapshot();
        let infos: Vec<BMRegistryInfoC> = records
            .into_iter()
            .filter_map(|r| r.info.map(registry_info_to_c))
            .collect();
        let len = infos.len();
        if len == 0 {
            unsafe {
                *out_ptr = std::ptr::null_mut();
                *out_len = 0;
            }
            return true;
        }
        let mut boxed = infos.into_boxed_slice();
        unsafe {
            *out_ptr = boxed.as_mut_ptr();
            *out_len = len;
        }
        std::mem::forget(boxed);
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_registry_snapshot_free(ptr: *mut BMRegistryInfoC, len: usize) {
    catch_void(|| {
        if ptr.is_null() || len == 0 {
            return;
        }
        unsafe {
            let slice = std::slice::from_raw_parts_mut(ptr, len);
            for r in slice.iter_mut() {
                bm_registry_info_free(r);
            }
            let _ = Box::from_raw(slice);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_handshake(out_ptr: *mut u8, out_len: usize) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len < 12 {
            return false;
        }
        let bytes = crate::codec::externals::handshake::Handshake::default_version().to_bytes();
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_ptr, 12);
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_registry_register(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    registry_info_ptr: *const BMRegistryInfoC,
    domain_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() || registry_info_ptr.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let reg = match unsafe { &*registry_info_ptr }.to_rust() {
            Some(v) => v,
            None => return false,
        };
        let domain = if domain_ptr.is_null() {
            None
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(domain_ptr) };
            match c_str.to_str() {
                Ok(s) => Some(s.to_owned()),
                Err(_) => return false,
            }
        };
        let actions = engine.make_registry_register(&dev_id, reg, domain);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_registry_list(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_registry_list(&dev_id);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_device_connect_requested(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    game_info_ptr: *const BMRegistryInfoC,
    controller_info_ptr: *const BMRegistryInfoC,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null()
            || out_actions.is_null()
            || game_info_ptr.is_null()
            || controller_info_ptr.is_null()
        {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let game = match unsafe { &*game_info_ptr }.to_rust() {
            Some(v) => v,
            None => return false,
        };
        let controller = match unsafe { &*controller_info_ptr }.to_rust() {
            Some(v) => v,
            None => return false,
        };
        let actions = engine.make_device_connect_requested(&dev_id, game, controller);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_registry_relay(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    dest_info_ptr: *const BMRegistryInfoC,
    inner_invoke_ptr: *const BMInvokeC,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null()
            || out_actions.is_null()
            || dest_info_ptr.is_null()
            || inner_invoke_ptr.is_null()
        {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let dest = match unsafe { &*dest_info_ptr }.to_rust() {
            Some(v) => v,
            None => return false,
        };
        let inner = match unsafe { &*inner_invoke_ptr }.to_rust() {
            Some(v) => v,
            None => return false,
        };
        let actions = engine.make_registry_relay(&dev_id, dest, inner);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_message_invoke(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    invoke_ptr: *const BMInvokeC,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() || invoke_ptr.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let inv = match unsafe { &*invoke_ptr }.to_rust() {
            Some(v) => v,
            None => return false,
        };
        let actions = engine.make_message_invoke(
            &dev_id,
            &inv.method,
            inv.return_method.as_deref(),
            inv.params,
        );
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
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

fn registry_info_to_c(
    src: crate::codec::externals::bm_registry_info::BMRegistryInfo,
) -> BMRegistryInfoC {
    let mut out = BMRegistryInfoC::default();
    out.slot_id = src.slot_id as i32;
    out.current_players = src.current_players.unwrap_or(0) as i32;
    out.max_players = src.max_players.unwrap_or(0) as i32;
    out.device_type_code = src.device.device_type.code();
    set_string_field(src.app_id, |p| {
        bm_registry_info_set_app_id_inner(&mut out, p)
    });
    set_string_field(src.device.device_id, |p| {
        bm_registry_info_set_device_id_inner(&mut out, p)
    });
    set_string_field(src.device.device_name, |p| {
        bm_registry_info_set_device_name_inner(&mut out, p)
    });
    let addr = src.device_address;
    set_string_field(addr.address, |p| {
        bm_registry_info_set_addr_inner(&mut out, p)
    });
    out.addr_unreliable_port = addr.unreliable_port;
    out.addr_reliable_port = addr.reliable_port;
    out
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_button_invoke(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    handler_ptr: *const c_char,
    pressed: bool,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() || handler_ptr.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let handler_c = unsafe { std::ffi::CStr::from_ptr(handler_ptr) };
        let handler = match handler_c.to_str() {
            Ok(s) => s,
            Err(_) => return false,
        };
        let actions = engine.make_button_invoke(&dev_id, handler, pressed);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_dpad_update(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    x: i16,
    y: i16,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_dpad_update(&dev_id, x, y);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_touch_set(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    touches_ptr: *const TouchPointC,
    touches_len: usize,
    reliability: i32,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        if touches_len > 0 && touches_ptr.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let mut touches = Vec::with_capacity(touches_len);
        if touches_len > 0 {
            let items = unsafe { std::slice::from_raw_parts(touches_ptr, touches_len) };
            for t in items {
                let state = match TouchState::from_value(t.state) {
                    Some(v) => v,
                    None => return false,
                };
                touches.push(Touch {
                    id: t.id,
                    x: t.x,
                    y: t.y,
                    screen_width: t.screen_width,
                    screen_height: t.screen_height,
                    state,
                });
            }
        }
        let actions = engine.make_touch_set(&dev_id, touches, reliability);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_accel(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    x: f64,
    y: f64,
    z: f64,
    reliability: i32,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_accel(&dev_id, x, y, z, reliability);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_gyro(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    x: f32,
    y: f32,
    z: f32,
    reliability: i32,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_gyro(&dev_id, x, y, z, reliability);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_orientation(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    x: f32,
    y: f32,
    z: f32,
    w: f32,
    reliability: i32,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }

        let engine = unsafe { &mut *ptr_engine };
        let target_device_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            c_str.to_string_lossy().into_owned()
        };

        let actions = engine.make_orientation(&target_device_id, x, y, z, w, reliability);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_request_xml(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    width: i32,
    height: i32,
    device_id_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let device_id = if device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };

        let actions = engine.make_request_xml(&target_id, width, height, &device_id);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_on_control_scheme_parsed(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    device_id_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let device_id = if device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };

        let actions = engine.make_on_control_scheme_parsed(&target_id, &device_id);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_simple_invoke(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    method_ptr: *const c_char,
    return_method_ptr: *const c_char,
    param_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() || method_ptr.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let method = {
            let c_str = unsafe { std::ffi::CStr::from_ptr(method_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let return_method = if return_method_ptr.is_null() {
            None
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(return_method_ptr) };
            match c_str.to_str() {
                Ok(s) => Some(s.to_owned()),
                Err(_) => return false,
            }
        };

        let param_str = if param_ptr.is_null() {
            None
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(param_ptr) };
            match c_str.to_str() {
                Ok(s) => Some(s.to_owned()),
                Err(_) => return false,
            }
        };

        let actions = engine.make_simple_invoke_string(
            &target_id,
            &method,
            return_method.as_deref(),
            param_str.as_deref(),
        );
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_vibrate(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_vibrate(&target_id);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_update_wallet(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_update_wallet(&target_id);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_controls_parse_xml(
    xml_ptr: *const c_char,
    out_len: *mut usize,
) -> *mut c_uchar {
    catch_ptr(|| {
        if xml_ptr.is_null() || out_len.is_null() {
            return std::ptr::null_mut();
        }
        let c_str = unsafe { std::ffi::CStr::from_ptr(xml_ptr) };
        let xml_str = match c_str.to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        };

        let mut parser = BMApplicationSchemeParser::new();
        let scheme = match parser.parse(xml_str.as_bytes()) {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        };

        let mut buf = Vec::new();
        if scheme.encode(&mut buf).is_err() {
            return std::ptr::null_mut();
        }

        let mut boxed_slice = buf.into_boxed_slice();
        let ptr = boxed_slice.as_mut_ptr();
        unsafe {
            *out_len = boxed_slice.len();
        }
        std::mem::forget(boxed_slice);
        ptr
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_controls_free_scheme_bytes(ptr: *mut c_uchar, len: usize) {
    catch_void(|| {
        if ptr.is_null() || len == 0 {
            return;
        }
        unsafe {
            let _ = Box::from_raw(std::slice::from_raw_parts_mut(ptr, len));
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_get_cookie(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    name_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() || name_ptr.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let name = {
            let c_str = unsafe { std::ffi::CStr::from_ptr(name_ptr) };
            match c_str.to_str() {
                Ok(s) => s,
                Err(_) => return false,
            }
        };
        let actions = engine.make_get_cookie(&target_id, name);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_set_cookie(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    name_ptr: *const c_char,
    value_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null()
            || out_actions.is_null()
            || name_ptr.is_null()
            || value_ptr.is_null()
        {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let name = {
            let c_str = unsafe { std::ffi::CStr::from_ptr(name_ptr) };
            match c_str.to_str() {
                Ok(s) => s,
                Err(_) => return false,
            }
        };
        let value = {
            let c_str = unsafe { std::ffi::CStr::from_ptr(value_ptr) };
            match c_str.to_str() {
                Ok(s) => s,
                Err(_) => return false,
            }
        };
        let actions = engine.make_set_cookie(&target_id, name, value);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_prompt_trial_upsell(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_prompt_trial_upsell(&target_id);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_wait_for_new_host(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    host_device_id_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() || host_device_id_ptr.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let host_device_id = {
            let c_str = unsafe { std::ffi::CStr::from_ptr(host_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s,
                Err(_) => return false,
            }
        };
        let actions = engine.make_wait_for_new_host(&target_id, host_device_id);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_set_control_mode(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    mode: i32,
    text_content_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let text_content = if text_content_ptr.is_null() {
            None
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(text_content_ptr) };
            match c_str.to_str() {
                Ok(s) => Some(s),
                Err(_) => return false,
            }
        };
        let actions = engine.make_set_control_mode(&target_id, mode, text_content);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_enable_accelerometer(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    enabled: bool,
    interval_seconds: f64,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let interval = if interval_seconds < 0.0 {
            None
        } else {
            Some(interval_seconds)
        };
        let actions = engine.make_enable_accelerometer(&target_id, enabled, interval);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_enable_touch(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    enabled: bool,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_enable_touch(&target_id, enabled);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_set_touch_interval(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    interval_seconds: f64,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_set_touch_interval(&target_id, interval_seconds);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_enable_gyro(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    enabled: bool,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_enable_gyro(&target_id, enabled);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_set_gyro_interval(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    interval_seconds: f64,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_set_gyro_interval(&target_id, interval_seconds);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_enable_orientation(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    enabled: bool,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_enable_orientation(&target_id, enabled);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_set_orientation_interval(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    interval_seconds: f64,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_set_orientation_interval(&target_id, interval_seconds);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_set_reliability_for_touch(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    touch_reliability: i32,
    control_reliability: i32,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_set_reliability_for_touch(
            &target_id,
            touch_reliability,
            control_reliability,
        );
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_set_capabilities(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    capabilities: u64,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_set_capabilities(&target_id, capabilities);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_safe_image_memory(ptr: *const u8, len: usize) -> *mut u8 {
    catch_ptr(|| {
        if ptr.is_null() || len == 0 {
            return std::ptr::null_mut();
        }
        let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
        let mut vec = Vec::with_capacity(len);
        vec.extend_from_slice(slice);
        let out = vec.as_mut_ptr();
        std::mem::forget(vec);
        out
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_free_image_memory(ptr: *mut u8, len: usize) {
    catch_void(|| {
        if ptr.is_null() {
            return;
        }
        unsafe {
            let _ = Vec::from_raw_parts(ptr, len, len);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_parse_control_scheme_proto(
    xml_ptr: *const u8,
    xml_len: usize,
    out_len: *mut usize,
) -> *mut u8 {
    catch_ptr(|| {
        if xml_ptr.is_null() || xml_len == 0 || out_len.is_null() {
            return std::ptr::null_mut();
        }
        let xml_data = unsafe { std::slice::from_raw_parts(xml_ptr, xml_len) };

        let mut parser = BMApplicationSchemeParser::new();
        match parser.parse(xml_data) {
            Ok(scheme) => {
                let mut buf = Vec::new();
                if scheme.encode(&mut buf).is_ok() {
                    let len = buf.len();
                    unsafe {
                        *out_len = len;
                    }
                    let ptr = buf.as_mut_ptr();
                    std::mem::forget(buf);
                    ptr
                } else {
                    std::ptr::null_mut()
                }
            }
            Err(_) => std::ptr::null_mut(),
        }
    })
}
