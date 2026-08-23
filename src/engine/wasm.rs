// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::config::EngineConfig;
use crate::controls::assembler::SchemeAssembler;
use crate::controls::parser::BMApplicationSchemeParser;
use crate::devices::bm_address::BMAddress;
use crate::devices::device_core::DeviceCore;
use crate::engine::device_registry::DeviceRecord;
use crate::engine::events::{Arrival, Command};
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

    pub fn declare_peer(
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
        let record = DeviceRecord::new(core, None);
        self.inner.push_registry_update(record);
        Ok(())
    }

    /// Everything this engine is told about itself, all at once.
    pub fn configure(&mut self, config: JsValue) -> Result<(), JsError> {
        let config: EngineConfig =
            serde_wasm_bindgen::from_value(config).map_err(|e| JsError::new(&e.to_string()))?;
        self.inner
            .configure(config)
            .map_err(|e| JsError::new(&e.to_string()))
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

    /// `arrival` is whatever the transport knows about where the bytes came
    /// from. A relayed transport has nothing to say and passes nothing.
    pub fn process_incoming(&mut self, data: &[u8], arrival: JsValue) -> Result<JsValue, JsError> {
        let arrival: Arrival = if arrival.is_undefined() || arrival.is_null() {
            Arrival::default()
        } else {
            serde_wasm_bindgen::from_value(arrival).map_err(|e| JsError::new(&e.to_string()))?
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.inner.process_incoming(data, &arrival)
        }));

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

fn version_from(
    parts: Option<Vec<u16>>,
) -> Result<Option<crate::codec::externals::bm_version::BMVersion>, JsError> {
    let Some(parts) = parts else {
        return Ok(None);
    };
    if parts.len() != 3 {
        return Err(JsError::new("a version is a major, a minor and a build"));
    }
    Ok(Some(crate::codec::externals::bm_version::BMVersion::new(
        parts[0] as u8,
        parts[1] as u8,
        parts[2],
    )))
}

/// Tracks the version exchange for one connection.
#[wasm_bindgen]
pub struct HandshakerWasm {
    inner: crate::link::negotiation::Handshaker,
}

#[wasm_bindgen]
impl HandshakerWasm {
    /// role is 0 to speak first, 1 to wait and answer.
    /// The versions default to the library's own; pass a pair to stand in as a
    /// different build.
    #[wasm_bindgen(constructor)]
    pub fn new(
        role: i32,
        current: Option<Vec<u16>>,
        minimum: Option<Vec<u16>>,
    ) -> Result<HandshakerWasm, JsError> {
        let role = crate::link::negotiation::LinkRole::from_code(role)
            .ok_or_else(|| JsError::new("unknown link role"))?;
        let inner = match (version_from(current)?, version_from(minimum)?) {
            (Some(current), Some(minimum)) => crate::link::negotiation::Handshaker::with_version(
                role,
                crate::codec::externals::handshake::Handshake::new(current, minimum),
            ),
            _ => crate::link::negotiation::Handshaker::new(role),
        };
        Ok(Self { inner })
    }

    /// What to send now the connection is up, empty when there is nothing.
    #[wasm_bindgen(js_name = onConnect)]
    pub fn on_connect(&mut self) -> Vec<u8> {
        self.inner.on_connect().unwrap_or_default()
    }

    #[wasm_bindgen(js_name = onMessage)]
    pub fn on_message(&mut self, data: &[u8]) -> Result<JsValue, JsError> {
        to_js(&self.inner.on_message(data))
    }

    #[wasm_bindgen(getter, js_name = isComplete)]
    pub fn is_complete(&self) -> bool {
        self.inner.is_complete()
    }

    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

/// The device type codes, for callers building a frame by hand. The engine
/// surface never asks for one.
#[wasm_bindgen(js_name = deviceTypeCodes)]
pub fn device_type_codes() -> Result<JsValue, JsError> {
    let obj = js_sys::Object::new();
    for kind in DeviceType::ALL {
        js_sys::Reflect::set(&obj, &kind.label().into(), &kind.code().into())
            .map_err(|_| JsError::new("could not build the device type table"))?;
    }
    Ok(obj.into())
}

/// The packet type codes, for callers building a frame by hand.
#[wasm_bindgen(js_name = packetTypeCodes)]
pub fn packet_type_codes() -> Result<JsValue, JsError> {
    let obj = js_sys::Object::new();
    for kind in crate::types::packet_type::PacketType::ALL {
        js_sys::Reflect::set(&obj, &kind.label().into(), &kind.code().into())
            .map_err(|_| JsError::new("could not build the packet type table"))?;
    }
    Ok(obj.into())
}

/// Whether these bytes open a cross domain policy request.
#[wasm_bindgen(js_name = isPolicyRequest)]
pub fn is_policy_request(data: &[u8]) -> bool {
    crate::link::crossdomain::is_policy_request(data)
}

/// The policy response to send back, NUL terminator included.
#[wasm_bindgen(js_name = policyResponse)]
pub fn policy_response() -> Vec<u8> {
    crate::link::crossdomain::RESPONSE.to_vec()
}

/// Watches the head of one connection for a policy request, for transports
/// that hand over bytes rather than let them be peeked.
#[wasm_bindgen]
pub struct PolicySnifferWasm {
    inner: crate::link::crossdomain::Sniffer,
}

#[wasm_bindgen]
impl PolicySnifferWasm {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: crate::link::crossdomain::Sniffer::new(),
        }
    }

    /// Offers the next bytes off the wire.
    pub fn feed(&mut self, data: &[u8]) -> Result<JsValue, JsError> {
        to_js(&self.inner.feed(data))
    }

    /// Whether the answer is still open. Once it is not, bytes can go straight
    /// on and the sniffer can be skipped for the rest of the connection.
    #[wasm_bindgen(getter, js_name = isWatching)]
    pub fn is_watching(&self) -> bool {
        self.inner.is_watching()
    }

    /// Whether the connection that just dropped was one we hung up on after
    /// answering. Watches again either way.
    #[wasm_bindgen(js_name = hungUp)]
    pub fn hung_up(&mut self) -> bool {
        self.inner.hung_up()
    }

    /// Starts over, for a sniffer reused across connections.
    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

impl Default for PolicySnifferWasm {
    fn default() -> Self {
        Self::new()
    }
}
