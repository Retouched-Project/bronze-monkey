// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::controls::assembler::SchemeAssembler;
use crate::controls::parser::BMApplicationSchemeParser;
use crate::devices::bm_address::BMAddress;
use crate::devices::device_core::DeviceCore;
use crate::engine::device_registry::DeviceRecord;
use crate::engine::events::Command;
use crate::policy::EndpointMode;
use crate::types::device_type::DeviceType;
use console_error_panic_hook;
use js_sys;
use prost::Message;
use serde_wasm_bindgen;
use std::panic::{AssertUnwindSafe, catch_unwind};
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsError, JsValue};

#[wasm_bindgen]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn parse_control_scheme_xml(xml_data: &str) -> Result<Vec<u8>, JsError> {
    let mut parser = BMApplicationSchemeParser::new();
    let scheme = parser
        .parse(xml_data.as_bytes())
        .map_err(|e| JsError::new(&e))?;

    let mut buf = Vec::with_capacity(scheme.encoded_len());
    scheme
        .encode(&mut buf)
        .map_err(|e| JsError::new(&e.to_string()))?;

    Ok(buf)
}

#[wasm_bindgen]
pub struct SchemeAssemblerWasm {
    inner: SchemeAssembler,
}

#[wasm_bindgen]
impl SchemeAssemblerWasm {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: SchemeAssembler::new(),
        }
    }

    pub fn offer(&mut self, set_id: &str, blob: &[u8]) -> Result<JsValue, JsError> {
        let result = self.inner.offer(set_id, blob);
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    pub fn current(&self) -> Option<Vec<u8>> {
        self.inner.current()
    }

    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

#[wasm_bindgen]
pub fn version_info() -> Result<JsValue, JsError> {
    to_js(&crate::version::version_info())
}

#[wasm_bindgen]
pub fn configure_logging(level: u8, capacity: u32) -> bool {
    crate::logging::install(crate::logging::LogConfig {
        level: crate::logging::level_filter_from_u8(level),
        capacity: capacity as usize,
    })
}

#[wasm_bindgen]
pub fn set_log_level(level: u8) {
    crate::logging::set_level(crate::logging::level_filter_from_u8(level));
}

#[wasm_bindgen]
pub fn take_logs() -> Result<JsValue, JsError> {
    to_js(&crate::logging::take_logs())
}

#[wasm_bindgen]
pub fn make_handshake_bytes() -> Vec<u8> {
    crate::codec::externals::handshake::Handshake::default_version()
        .to_bytes()
        .to_vec()
}

#[wasm_bindgen]
pub fn generate_device_id() -> String {
    crate::identity::generate_device_id()
}

#[wasm_bindgen]
pub fn generate_app_id() -> String {
    crate::identity::generate_app_id()
}

#[wasm_bindgen]
pub struct BmEngineWasm {
    inner: crate::engine::processing::Engine,
}

#[wasm_bindgen]
impl BmEngineWasm {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: crate::engine::processing::Engine::new(),
        }
    }

    pub fn init_local_device(
        &mut self,
        id: &str,
        name: &str,
        type_code: i32,
        address: &str,
        unreliable_port: i32,
        reliable_port: i32,
    ) -> Result<(), JsError> {
        let dt = DeviceType::for_value(type_code).map_err(|e| JsError::new(&e.to_string()))?;
        let mut core = DeviceCore::new(id.to_string(), name.to_string(), dt);
        core.address = Some(BMAddress {
            address: address.to_string(),
            unreliable_port,
            reliable_port,
        });
        self.inner.init_local_device(core);
        Ok(())
    }

    pub fn register_device(
        &mut self,
        id: &str,
        name: &str,
        type_code: i32,
        address: &str,
        unreliable_port: i32,
        reliable_port: i32,
    ) -> Result<(), JsError> {
        let dt = DeviceType::for_value(type_code).map_err(|e| JsError::new(&e.to_string()))?;
        let mut core = DeviceCore::new(id.to_string(), name.to_string(), dt);
        core.address = Some(BMAddress {
            address: address.to_string(),
            unreliable_port,
            reliable_port,
        });
        let record = DeviceRecord::new(core, None, None);
        self.inner.push_registry_update(record);
        Ok(())
    }

    pub fn set_auto_approve_registration(&mut self, value: bool) {
        self.inner.server_policy.auto_approve_registration = value;
    }

    pub fn configure_roles(&mut self, server_enabled: bool, endpoint_mode: i32) {
        let endpoint = match endpoint_mode {
            1 => Some(EndpointMode::Game),
            2 => Some(EndpointMode::Controller),
            _ => None,
        };
        self.inner.configure_roles(server_enabled, endpoint);
    }

    pub fn approve_registration(&mut self, device_id: &str) -> Result<JsValue, JsError> {
        let outgoings = self.inner.approve_registration(device_id);
        to_js(&outgoings)
    }

    pub fn deny_registration(&mut self, device_id: &str) -> Result<JsValue, JsError> {
        let outgoings = self.inner.deny_registration(device_id);
        to_js(&outgoings)
    }

    pub fn get_registry(&self) -> Result<JsValue, JsError> {
        let records = self.inner.registry().snapshot();
        let array = js_sys::Array::new();
        for record in records {
            let obj = js_sys::Object::new();
            js_sys::Reflect::set(
                &obj,
                &"deviceId".into(),
                &record.core.device_id.clone().into(),
            )
            .unwrap();
            js_sys::Reflect::set(
                &obj,
                &"deviceName".into(),
                &record.core.device_name.clone().into(),
            )
            .unwrap();
            js_sys::Reflect::set(
                &obj,
                &"deviceType".into(),
                &record.core.device_type.code().into(),
            )
            .unwrap();
            if let Some(class_id) = record.class_id {
                js_sys::Reflect::set(&obj, &"classId".into(), &(class_id as i32).into()).unwrap();
            }
            if let Some(addr) = &record.core.address {
                let addr_obj = js_sys::Object::new();
                js_sys::Reflect::set(&addr_obj, &"address".into(), &addr.address.clone().into())
                    .unwrap();
                js_sys::Reflect::set(
                    &addr_obj,
                    &"reliable_port".into(),
                    &addr.reliable_port.into(),
                )
                .unwrap();
                js_sys::Reflect::set(
                    &addr_obj,
                    &"unreliable_port".into(),
                    &addr.unreliable_port.into(),
                )
                .unwrap();
                js_sys::Reflect::set(&obj, &"address".into(), &addr_obj).unwrap();
            }
            if let Some(info) = &record.info {
                js_sys::Reflect::set(&obj, &"slotId".into(), &info.slot_id.into()).unwrap();
                js_sys::Reflect::set(&obj, &"appId".into(), &info.app_id.clone().into()).unwrap();
                if let Some(cp) = info.current_players {
                    js_sys::Reflect::set(&obj, &"currentPlayers".into(), &cp.into()).unwrap();
                }
                if let Some(mp) = info.max_players {
                    js_sys::Reflect::set(&obj, &"maxPlayers".into(), &mp.into()).unwrap();
                }
            }
            array.push(&obj);
        }
        Ok(array.into())
    }

    pub fn process_incoming(&mut self, data: &[u8]) -> Result<JsValue, JsError> {
        let result = catch_unwind(AssertUnwindSafe(|| self.inner.process_incoming(data)));

        match result {
            Ok(out) => to_js(&out),
            Err(_) => {
                log::error!("panic in process_incoming");
                Err(JsError::new("Rust panic in process_incoming"))
            }
        }
    }

    pub fn emit(&mut self, command: JsValue) -> Result<JsValue, JsError> {
        let cmd: Command =
            serde_wasm_bindgen::from_value(command).map_err(|e| JsError::new(&e.to_string()))?;
        let outgoings = self.inner.emit(cmd);
        to_js(&outgoings)
    }

    pub fn register_button_handlers(&mut self, handlers: Vec<String>) {
        self.inner.register_button_handlers(handlers);
    }

    pub fn clear_button_handlers(&mut self) {
        self.inner.clear_button_handlers();
    }
}

fn to_js<T: serde::Serialize>(value: &T) -> Result<JsValue, JsError> {
    serde_wasm_bindgen::to_value(value).map_err(|e| JsError::new(&e.to_string()))
}

/// Reads a frame into its described form. Engine free: it allocates no sequence
/// numbers and needs no registered device, so it can run alongside a live
/// session without disturbing it.
#[wasm_bindgen]
pub fn inspect_wire(data: &[u8]) -> Result<JsValue, JsError> {
    let view = crate::inspect::inspect(data).map_err(|e| JsError::new(&e.to_string()))?;
    to_js(&view)
}

/// Serializes a described frame into wire bytes.
#[wasm_bindgen]
pub fn build_wire(view: JsValue) -> Result<Vec<u8>, JsError> {
    let view: crate::inspect::WireView =
        serde_wasm_bindgen::from_value(view).map_err(|e| JsError::new(&e.to_string()))?;
    crate::inspect::build(view).map_err(|e| JsError::new(&e.to_string()))
}

/// Reassembles messages from a stream that arrives in arbitrary pieces.
#[wasm_bindgen]
pub struct FramerWasm {
    inner: crate::link::framing::Framer,
}

#[wasm_bindgen]
impl FramerWasm {
    /// Rejects messages longer than maxLen, or the library ceiling when it is
    /// left out. A limit above that ceiling is clamped to it.
    #[wasm_bindgen(constructor)]
    pub fn new(max_len: Option<u32>) -> Self {
        let max_len = max_len.map_or(crate::link::framing::MAX_MESSAGE_LEN, |n| n as usize);
        Self {
            inner: crate::link::framing::Framer::with_max_len(max_len),
        }
    }

    /// The limit this framer was created with.
    #[wasm_bindgen(getter, js_name = maxLen)]
    pub fn max_len(&self) -> usize {
        self.inner.max_len()
    }

    /// Adds bytes and returns every message they completed.
    pub fn feed(&mut self, data: &[u8]) -> Result<js_sys::Array, JsError> {
        let messages = self
            .inner
            .feed(data)
            .map_err(|e| JsError::new(&e.to_string()))?;
        let out = js_sys::Array::new_with_length(messages.len() as u32);
        for (i, message) in messages.iter().enumerate() {
            out.set(
                i as u32,
                js_sys::Uint8Array::from(message.as_slice()).into(),
            );
        }
        Ok(out)
    }

    /// Drops anything half read, for when a connection restarts.
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    #[wasm_bindgen(getter)]
    pub fn pending(&self) -> usize {
        self.inner.pending()
    }
}

impl Default for FramerWasm {
    fn default() -> Self {
        Self::new(None)
    }
}

/// The longest message the library will accept.
#[wasm_bindgen]
pub fn max_message_len() -> usize {
    crate::link::framing::MAX_MESSAGE_LEN
}

/// Writes a message with the length prefix a stream transport needs.
#[wasm_bindgen]
pub fn frame(message: &[u8]) -> Vec<u8> {
    crate::link::framing::frame(message)
}
