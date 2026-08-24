// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use prost::Message;

use crate::config::EngineConfig;
use crate::controls::assembler::{SchemeAssembler, SchemeOffer};
use crate::controls::parser::BMApplicationSchemeParser;
use crate::devices::device_core::DeviceCore;
use crate::engine::device_registry::DeviceRecord;
use crate::engine::events::{Arrival, Command};
use crate::engine::processing::Engine;
use crate::link::crossdomain::Sniffer;
use crate::link::framing::Framer;
use crate::link::negotiation::{Handshaker, LinkRole};

use super::{catch_bool, catch_i32, catch_ptr, catch_usize, catch_void};

fn write_buf(buf: Vec<u8>, out_ptr: *mut *mut u8, out_len: *mut usize) {
    if out_ptr.is_null() || out_len.is_null() {
        return;
    }
    if buf.is_empty() {
        unsafe {
            *out_ptr = std::ptr::null_mut();
            *out_len = 0;
        }
        return;
    }
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

fn assembler_mut<'a>(ptr: *mut SchemeAssembler) -> Option<&'a mut SchemeAssembler> {
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
            let slice_ptr = std::ptr::slice_from_raw_parts_mut(ptr, len);
            let _ = Box::from_raw(slice_ptr);
        }
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_version_info(out_ptr: *mut *mut u8, out_len: *mut usize) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        match rmp_serde::to_vec_named(&crate::version::version_info()) {
            Ok(buf) => {
                write_buf(buf, out_ptr, out_len);
                true
            }
            Err(_) => false,
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_generate_device_id(out_ptr: *mut *mut u8, out_len: *mut usize) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        write_buf(
            crate::identity::generate_device_id().into_bytes(),
            out_ptr,
            out_len,
        );
        true
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_generate_app_id(out_ptr: *mut *mut u8, out_len: *mut usize) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        write_buf(
            crate::identity::generate_app_id().into_bytes(),
            out_ptr,
            out_len,
        );
        true
    })
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
pub unsafe extern "C" fn bm_engine_declare_peer(
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
        engine.registry_mut().upsert(DeviceRecord::new(core, None));
        true
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_engine_process_incoming(
    ptr_engine: *mut Engine,
    payload: *const u8,
    payload_len: usize,
    arrival: *const u8,
    arrival_len: usize,
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
        // A caller with nothing to say about the transport passes no bytes.
        let arrival = match arrival_len {
            0 => Arrival::default(),
            _ => match rmp_serde::from_slice(in_slice(arrival, arrival_len)) {
                Ok(a) => a,
                Err(_) => return false,
            },
        };
        let out = engine.process_incoming(in_slice(payload, payload_len), &arrival);
        match rmp_serde::to_vec_named(&out) {
            Ok(buf) => {
                write_buf(buf, out_ptr, out_len);
                true
            }
            Err(_) => false,
        }
    })
}

/// Tells the engine what time it is, in milliseconds on any monotonic clock
/// the caller prefers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_engine_handle_time(
    ptr_engine: *mut Engine,
    now_ms: u64,
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
        match rmp_serde::to_vec_named(&engine.handle_time(now_ms)) {
            Ok(buf) => {
                write_buf(buf, out_ptr, out_len);
                true
            }
            Err(e) => {
                crate::set_last_error(e);
                false
            }
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
            Err(e) => {
                crate::set_last_error(e);
                return false;
            }
        };
        let outgoings = match engine.emit(cmd) {
            Ok(o) => o,
            Err(e) => {
                crate::set_last_error(e);
                return false;
            }
        };
        match rmp_serde::to_vec_named(&outgoings) {
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

/// Applies a msgpack encoded EngineConfig: everything this engine is told about
/// itself, all at once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_engine_configure(
    ptr_engine: *mut Engine,
    mp_ptr: *const u8,
    mp_len: usize,
) -> bool {
    catch_bool(|| {
        let Some(engine) = engine_mut(ptr_engine) else {
            return false;
        };
        let config: EngineConfig = match rmp_serde::from_slice(in_slice(mp_ptr, mp_len)) {
            Ok(c) => c,
            Err(e) => {
                crate::set_last_error(e);
                return false;
            }
        };
        match engine.configure(config) {
            Ok(()) => true,
            Err(e) => {
                crate::set_last_error(e);
                false
            }
        }
    })
}

/// Takes a msgpack encoded list of handler names.
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
            Err(e) => {
                crate::set_last_error(e);
                return false;
            }
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
pub extern "C" fn bm_scheme_assembler_new() -> *mut SchemeAssembler {
    catch_ptr(|| Box::into_raw(Box::new(SchemeAssembler::new())))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_scheme_assembler_free(ptr: *mut SchemeAssembler) {
    catch_void(|| {
        if ptr.is_null() {
            return;
        }
        drop(unsafe { Box::from_raw(ptr) });
    });
}

/// Offers a completed chunk set. Returns 0 when the set is not a control scheme
/// (the caller owns the blob), 1 when consumed with nothing new, 2 when updated
/// (the merged scheme is written to out_scheme and out_initial is set), or -1 on
/// error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_scheme_assembler_offer(
    ptr: *mut SchemeAssembler,
    set_id_ptr: *const u8,
    set_id_len: usize,
    blob_ptr: *const u8,
    blob_len: usize,
    out_scheme_ptr: *mut *mut u8,
    out_scheme_len: *mut usize,
    out_initial: *mut bool,
) -> i32 {
    catch_i32(|| {
        let Some(assembler) = assembler_mut(ptr) else {
            return -1;
        };
        let set_id = std::str::from_utf8(in_slice(set_id_ptr, set_id_len)).unwrap_or("");
        match assembler.offer(set_id, in_slice(blob_ptr, blob_len)) {
            SchemeOffer::Updated(update) => {
                if !out_scheme_ptr.is_null() && !out_scheme_len.is_null() {
                    write_buf(update.scheme, out_scheme_ptr, out_scheme_len);
                }
                if !out_initial.is_null() {
                    unsafe { *out_initial = update.initial };
                }
                2
            }
            SchemeOffer::Consumed => 1,
            SchemeOffer::NotScheme => 0,
        }
    })
}

/// Writes the current merged scheme to out and returns true, or returns false
/// when no scheme has been assembled yet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_scheme_assembler_current(
    ptr: *mut SchemeAssembler,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        let Some(assembler) = assembler_mut(ptr) else {
            return false;
        };
        match assembler.current() {
            Some(bytes) => {
                write_buf(bytes, out_ptr, out_len);
                true
            }
            None => false,
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_scheme_assembler_reset(ptr: *mut SchemeAssembler) {
    catch_void(|| {
        if let Some(assembler) = assembler_mut(ptr) {
            assembler.reset();
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_log_configure(level: u8, capacity: usize) -> bool {
    catch_bool(|| {
        crate::logging::install(crate::logging::LogConfig {
            level: crate::logging::level_filter_from_u8(level),
            capacity,
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_log_set_level(level: u8) -> bool {
    catch_bool(|| {
        crate::logging::set_level(crate::logging::level_filter_from_u8(level));
        true
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_log_take(out_ptr: *mut *mut u8, out_len: *mut usize) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        match rmp_serde::to_vec_named(&crate::logging::take_logs()) {
            Ok(buf) => {
                write_buf(buf, out_ptr, out_len);
                true
            }
            Err(_) => false,
        }
    })
}

/// Reads a frame into a msgpack encoded WireView. Engine free: it allocates no
/// sequence numbers and needs no registered device, so it can run alongside a
/// live session without disturbing it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_inspect_wire(
    data_ptr: *const u8,
    data_len: usize,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        let view = match crate::inspect::inspect(in_slice(data_ptr, data_len)) {
            Ok(v) => v,
            Err(e) => {
                crate::set_last_error(e);
                return false;
            }
        };
        match rmp_serde::to_vec_named(&view) {
            Ok(buf) => {
                write_buf(buf, out_ptr, out_len);
                true
            }
            Err(e) => {
                crate::set_last_error(e);
                false
            }
        }
    })
}

/// Serializes a msgpack encoded WireView into wire bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_build_wire(
    view_ptr: *const u8,
    view_len: usize,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        let view: crate::inspect::WireView =
            match rmp_serde::from_slice(in_slice(view_ptr, view_len)) {
                Ok(v) => v,
                Err(e) => {
                    crate::set_last_error(e);
                    return false;
                }
            };
        match crate::inspect::build(view) {
            Ok(buf) => {
                write_buf(buf, out_ptr, out_len);
                true
            }
            Err(e) => {
                crate::set_last_error(e);
                false
            }
        }
    })
}

fn framer_mut<'a>(ptr: *mut Framer) -> Option<&'a mut Framer> {
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &mut *ptr })
    }
}

/// The longest message the library will accept, for callers that want to pass
/// it back to bm_framer_new rather than choose a limit of their own.
#[unsafe(no_mangle)]
pub extern "C" fn bm_max_message_len() -> usize {
    crate::link::framing::MAX_MESSAGE_LEN
}

/// Creates a framer that rejects messages longer than max_len. A limit above
/// the library ceiling is clamped to it.
#[unsafe(no_mangle)]
pub extern "C" fn bm_framer_new(max_len: usize) -> *mut Framer {
    catch_ptr(|| Box::into_raw(Box::new(Framer::with_max_len(max_len))))
}

/// The limit this framer was created with.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_framer_max_len(ptr: *mut Framer) -> usize {
    catch_usize(|| framer_mut(ptr).map_or(0, |f| f.max_len()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_framer_free(ptr: *mut Framer) {
    catch_void(|| {
        if !ptr.is_null() {
            drop(unsafe { Box::from_raw(ptr) });
        }
    })
}

/// Feeds stream bytes and writes back every completed message, msgpack encoded
/// as an array of byte strings. Returns false when the stream is unusable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_framer_feed(
    ptr: *mut Framer,
    data_ptr: *const u8,
    data_len: usize,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        let Some(framer) = framer_mut(ptr) else {
            return false;
        };
        let messages = match framer.feed(in_slice(data_ptr, data_len)) {
            Ok(m) => m,
            Err(e) => {
                crate::set_last_error(e);
                return false;
            }
        };
        match rmp_serde::to_vec_named(&messages) {
            Ok(buf) => {
                write_buf(buf, out_ptr, out_len);
                true
            }
            Err(e) => {
                crate::set_last_error(e);
                false
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_framer_reset(ptr: *mut Framer) {
    catch_void(|| {
        if let Some(framer) = framer_mut(ptr) {
            framer.reset();
        }
    })
}

/// Writes a message with the length prefix a stream transport needs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_frame(
    data_ptr: *const u8,
    data_len: usize,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        write_buf(
            crate::link::framing::frame(in_slice(data_ptr, data_len)),
            out_ptr,
            out_len,
        );
        true
    })
}

fn handshaker_mut<'a>(ptr: *mut Handshaker) -> Option<&'a mut Handshaker> {
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &mut *ptr })
    }
}

/// Creates a version negotiator for one connection. role is 0 to speak first,
/// 1 to wait and answer.
#[unsafe(no_mangle)]
pub extern "C" fn bm_handshaker_new(role: i32) -> *mut Handshaker {
    catch_ptr(|| match LinkRole::from_code(role) {
        Some(role) => Box::into_raw(Box::new(Handshaker::new(role))),
        None => std::ptr::null_mut(),
    })
}

/// Creates one that announces versions other than the library's own, for a
/// caller standing in as a different build.
#[unsafe(no_mangle)]
pub extern "C" fn bm_handshaker_new_with_version(
    role: i32,
    current_major: u8,
    current_minor: u8,
    current_build: u16,
    minimum_major: u8,
    minimum_minor: u8,
    minimum_build: u16,
) -> *mut Handshaker {
    catch_ptr(|| match LinkRole::from_code(role) {
        Some(role) => {
            let local = crate::codec::externals::handshake::Handshake::new(
                crate::codec::externals::bm_version::BMVersion::new(
                    current_major,
                    current_minor,
                    current_build,
                ),
                crate::codec::externals::bm_version::BMVersion::new(
                    minimum_major,
                    minimum_minor,
                    minimum_build,
                ),
            );
            Box::into_raw(Box::new(Handshaker::with_version(role, local)))
        }
        None => std::ptr::null_mut(),
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_handshaker_free(ptr: *mut Handshaker) {
    catch_void(|| {
        if !ptr.is_null() {
            drop(unsafe { Box::from_raw(ptr) });
        }
    })
}

/// Writes what to send now that the connection is up. An empty result means
/// there is nothing to send, which is the normal case for a responder.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_handshaker_on_connect(
    ptr: *mut Handshaker,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        let Some(handshaker) = handshaker_mut(ptr) else {
            return false;
        };
        write_buf(
            handshaker.on_connect().unwrap_or_default(),
            out_ptr,
            out_len,
        );
        true
    })
}

/// Classifies one message, writing back a msgpack encoded HandshakeOutcome.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_handshaker_on_message(
    ptr: *mut Handshaker,
    data_ptr: *const u8,
    data_len: usize,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        let Some(handshaker) = handshaker_mut(ptr) else {
            return false;
        };
        let outcome = handshaker.on_message(in_slice(data_ptr, data_len));
        match rmp_serde::to_vec_named(&outcome) {
            Ok(buf) => {
                write_buf(buf, out_ptr, out_len);
                true
            }
            Err(e) => {
                crate::set_last_error(e);
                false
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_handshaker_is_complete(ptr: *mut Handshaker) -> bool {
    catch_bool(|| handshaker_mut(ptr).is_some_and(|h| h.is_complete()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_handshaker_reset(ptr: *mut Handshaker) {
    catch_void(|| {
        if let Some(handshaker) = handshaker_mut(ptr) {
            handshaker.reset();
        }
    })
}

fn write_code_table(
    table: std::collections::BTreeMap<&'static str, i32>,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> bool {
    if out_ptr.is_null() || out_len.is_null() {
        return false;
    }
    match rmp_serde::to_vec_named(&table) {
        Ok(buf) => {
            write_buf(buf, out_ptr, out_len);
            true
        }
        Err(e) => {
            crate::set_last_error(e);
            false
        }
    }
}

/// Writes the device type codes as a msgpack map, for callers building a frame
/// by hand. The engine surface never asks for one.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_device_type_codes(out_ptr: *mut *mut u8, out_len: *mut usize) -> bool {
    catch_bool(|| {
        write_code_table(
            crate::types::device_type::DeviceType::ALL
                .iter()
                .map(|k| (k.label(), k.code()))
                .collect(),
            out_ptr,
            out_len,
        )
    })
}

/// Writes the packet type codes as a msgpack map.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_packet_type_codes(out_ptr: *mut *mut u8, out_len: *mut usize) -> bool {
    catch_bool(|| {
        write_code_table(
            crate::types::packet_type::PacketType::ALL
                .iter()
                .map(|k| (k.label(), k.code()))
                .collect(),
            out_ptr,
            out_len,
        )
    })
}

/// Whether these bytes open a cross domain policy request.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_is_policy_request(data_ptr: *const u8, data_len: usize) -> bool {
    catch_bool(|| crate::link::crossdomain::is_policy_request(in_slice(data_ptr, data_len)))
}

/// Writes the policy response to send back, NUL terminator included.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_policy_response(out_ptr: *mut *mut u8, out_len: *mut usize) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        write_buf(
            crate::link::crossdomain::RESPONSE.to_vec(),
            out_ptr,
            out_len,
        );
        true
    })
}

fn sniffer_mut<'a>(ptr: *mut Sniffer) -> Option<&'a mut Sniffer> {
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &mut *ptr })
    }
}

/// Creates a sniffer that watches the head of one connection for a policy
/// request, for transports that hand over bytes rather than let them be peeked.
#[unsafe(no_mangle)]
pub extern "C" fn bm_policy_sniffer_new() -> *mut Sniffer {
    catch_ptr(|| Box::into_raw(Box::new(Sniffer::new())))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_policy_sniffer_free(ptr: *mut Sniffer) {
    catch_void(|| {
        if !ptr.is_null() {
            drop(unsafe { Box::from_raw(ptr) });
        }
    })
}

/// Offers the next bytes off the wire, writing back a msgpack encoded Sniff.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_policy_sniffer_feed(
    ptr: *mut Sniffer,
    data_ptr: *const u8,
    data_len: usize,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> bool {
    catch_bool(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return false;
        }
        let Some(sniffer) = sniffer_mut(ptr) else {
            return false;
        };
        let sniff = sniffer.feed(in_slice(data_ptr, data_len));
        match rmp_serde::to_vec_named(&sniff) {
            Ok(buf) => {
                write_buf(buf, out_ptr, out_len);
                true
            }
            Err(e) => {
                crate::set_last_error(e);
                false
            }
        }
    })
}

/// Whether the answer is still open. Once it is not, bytes can go straight on.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_policy_sniffer_is_watching(ptr: *mut Sniffer) -> bool {
    catch_bool(|| sniffer_mut(ptr).is_some_and(|s| s.is_watching()))
}

/// Whether the connection that just dropped was one we hung up on after
/// answering. Watches again either way.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_policy_sniffer_hung_up(ptr: *mut Sniffer) -> bool {
    catch_bool(|| sniffer_mut(ptr).is_some_and(|s| s.hung_up()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_policy_sniffer_reset(ptr: *mut Sniffer) {
    catch_void(|| {
        if let Some(sniffer) = sniffer_mut(ptr) {
            sniffer.reset();
        }
    })
}
