// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use prost::Message;

use crate::controls::ControlScheme;
use crate::controls::parser::BMApplicationSchemeParser;
use crate::devices::device_core::DeviceCore;
use crate::engine::events::Command;
use crate::engine::processing::Engine;
use crate::engine::registry::DeviceRecord;
use crate::policy::Role;

use super::{catch_bool, catch_ptr, catch_void};

fn write_buf(buf: Vec<u8>, out_ptr: *mut *mut u8, out_len: *mut usize) {
    let mut boxed = buf.into_boxed_slice();
    unsafe {
        *out_ptr = boxed.as_mut_ptr();
        *out_len = boxed.len();
    }
    std::mem::forget(boxed);
}

fn in_slice<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if ptr.is_null() || len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

fn engine_mut<'a>(ptr: *mut Engine) -> Option<&'a mut Engine> {
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &mut *ptr })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_buffer_free(ptr: *mut u8, len: usize) {
    catch_void(|| {
        if ptr.is_null() || len == 0 {
            return;
        }
        unsafe {
            let _ = Box::from_raw(std::slice::from_raw_parts_mut(ptr, len));
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_new() -> *mut Engine {
    catch_ptr(|| Box::into_raw(Box::new(Engine::new())))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_engine_free(ptr_engine: *mut Engine) {
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
pub unsafe extern "C" fn bm_engine_init_local_device(
    ptr_engine: *mut Engine,
    mp_ptr: *const u8,
    mp_len: usize,
) -> bool {
    catch_bool(|| {
        let Some(engine) = engine_mut(ptr_engine) else {
            return false;
        };
        let core: DeviceCore = match rmp_serde::from_slice(in_slice(mp_ptr, mp_len)) {
            Ok(c) => c,
            Err(_) => return false,
        };
        engine.init_local_device(core);
        true
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_engine_register_device(
    ptr_engine: *mut Engine,
    mp_ptr: *const u8,
    mp_len: usize,
) -> bool {
    catch_bool(|| {
        let Some(engine) = engine_mut(ptr_engine) else {
            return false;
        };
        let core: DeviceCore = match rmp_serde::from_slice(in_slice(mp_ptr, mp_len)) {
            Ok(c) => c,
            Err(_) => return false,
        };
        engine
            .registry_mut()
            .upsert(DeviceRecord::new(core, None, None));
        true
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_engine_process_incoming(
    ptr_engine: *mut Engine,
    payload: *const u8,
    payload_len: usize,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        let Some(engine) = engine_mut(ptr_engine) else {
            return false;
        };
        let out = engine.process_incoming(in_slice(payload, payload_len));
        match rmp_serde::to_vec_named(&out) {
            Ok(buf) => {
                write_buf(buf, out_ptr, out_len);
                true
            }
            Err(_) => false,
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_engine_process_incoming_udp(
    ptr_engine: *mut Engine,
    payload: *const u8,
    payload_len: usize,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        let Some(engine) = engine_mut(ptr_engine) else {
            return false;
        };
        let out = engine.process_incoming_udp(in_slice(payload, payload_len));
        match rmp_serde::to_vec_named(&out) {
            Ok(buf) => {
                write_buf(buf, out_ptr, out_len);
                true
            }
            Err(_) => false,
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_engine_emit(
    ptr_engine: *mut Engine,
    cmd_ptr: *const u8,
    cmd_len: usize,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        let Some(engine) = engine_mut(ptr_engine) else {
            return false;
        };
        let cmd: Command = match rmp_serde::from_slice(in_slice(cmd_ptr, cmd_len)) {
            Ok(c) => c,
            Err(_) => return false,
        };
        match rmp_serde::to_vec_named(&engine.emit(cmd)) {
            Ok(buf) => {
                write_buf(buf, out_ptr, out_len);
                true
            }
            Err(_) => false,
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_engine_registry(
    ptr_engine: *mut Engine,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        let Some(engine) = engine_mut(ptr_engine) else {
            return false;
        };
        match rmp_serde::to_vec_named(&engine.registry().snapshot()) {
            Ok(buf) => {
                write_buf(buf, out_ptr, out_len);
                true
            }
            Err(_) => false,
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_engine_set_role_enabled(
    ptr_engine: *mut Engine,
    role_code: i32,
    enabled: bool,
) -> bool {
    catch_bool(|| {
        let Some(engine) = engine_mut(ptr_engine) else {
            return false;
        };
        let role = match role_code {
            0 => Role::Server,
            1 => Role::Game,
            2 => Role::Controller,
            _ => return false,
        };
        engine.set_role_enabled(role, enabled);
        true
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_engine_set_auto_approve_registration(
    ptr_engine: *mut Engine,
    value: bool,
) -> bool {
    catch_bool(|| {
        let Some(engine) = engine_mut(ptr_engine) else {
            return false;
        };
        engine.server_policy.auto_approve_registration = value;
        true
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_engine_register_button_handlers(
    ptr_engine: *mut Engine,
    mp_ptr: *const u8,
    mp_len: usize,
) -> bool {
    catch_bool(|| {
        let Some(engine) = engine_mut(ptr_engine) else {
            return false;
        };
        let handlers: Vec<String> = match rmp_serde::from_slice(in_slice(mp_ptr, mp_len)) {
            Ok(h) => h,
            Err(_) => return false,
        };
        engine.register_button_handlers(handlers);
        true
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_engine_clear_button_handlers(ptr_engine: *mut Engine) -> bool {
    catch_bool(|| {
        let Some(engine) = engine_mut(ptr_engine) else {
            return false;
        };
        engine.clear_button_handlers();
        true
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_engine_handshake(out_ptr: *mut u8, out_len: usize) -> bool {
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
pub unsafe extern "C" fn bm_controls_parse_xml(
    xml_ptr: *const u8,
    xml_len: usize,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        let mut parser = BMApplicationSchemeParser::new();
        let scheme = match parser.parse(in_slice(xml_ptr, xml_len)) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let mut buf = Vec::new();
        if scheme.encode(&mut buf).is_err() {
            return false;
        }
        write_buf(buf, out_ptr, out_len);
        true
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_controls_merge_scheme(
    base_ptr: *const u8,
    base_len: usize,
    update_ptr: *const u8,
    update_len: usize,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        let mut base = match ControlScheme::decode(in_slice(base_ptr, base_len)) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let update = match ControlScheme::decode(in_slice(update_ptr, update_len)) {
            Ok(s) => s,
            Err(_) => return false,
        };
        crate::controls::merge::apply_update(&mut base, update);
        let mut buf = Vec::with_capacity(base.encoded_len());
        if base.encode(&mut buf).is_err() {
            return false;
        }
        write_buf(buf, out_ptr, out_len);
        true
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_safe_image_memory(
    in_ptr: *const u8,
    in_len: usize,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() || in_ptr.is_null() || in_len == 0 {
            return false;
        }
        let src = unsafe { std::slice::from_raw_parts(in_ptr, in_len) };
        write_buf(src.to_vec(), out_ptr, out_len);
        true
    })
}
