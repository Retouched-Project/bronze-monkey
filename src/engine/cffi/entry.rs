// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use prost::Message;
use std::os::raw::{c_char, c_uchar};

use crate::codec::externals::bm_registry_info::{BMRegistryInfoC, bm_registry_info_free};
use crate::controls::parser::BMApplicationSchemeParser;
use crate::devices::device_core::DeviceCoreC;
use crate::engine::processing::Engine;
use crate::engine::registry::DeviceRecord;

use super::marshal_in::*;
use super::marshal_out::*;
use super::types::*;
use super::*;

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
pub extern "C" fn bm_engine_emit(
    ptr_engine: *mut Engine,
    command: *const CommandC,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || command.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let cmd = match command_from_c(unsafe { &*command }) {
            Some(c) => c,
            None => return false,
        };
        let actions = engine.emit(cmd);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_register_button_handlers(
    ptr_engine: *mut Engine,
    handlers_ptr: *const *const c_char,
    handlers_len: usize,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        if handlers_len == 0 {
            return true;
        }
        if handlers_ptr.is_null() {
            return false;
        }
        let items = unsafe { std::slice::from_raw_parts(handlers_ptr, handlers_len) };
        let mut handlers = Vec::with_capacity(handlers_len);
        for &p in items {
            match req_str(p) {
                Some(s) => handlers.push(s),
                None => return false,
            }
        }
        engine.register_button_handlers(handlers);
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_clear_button_handlers(ptr_engine: *mut Engine) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        engine.clear_button_handlers();
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
