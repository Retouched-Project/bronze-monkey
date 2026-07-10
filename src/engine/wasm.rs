// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::bm_registry_info::BMRegistryInfo;
use crate::codec::messages::touch::Touch;
use crate::controls::assembler::SchemeAssembler;
use crate::controls::parser::BMApplicationSchemeParser;
use crate::devices::bm_address::BMAddress;
use crate::devices::device_core::DeviceCore;
use crate::engine::events::Command;
use crate::engine::registry::DeviceRecord;
use crate::policy::Role;
use crate::types::channel_type::ChannelType;
use crate::types::control_mode::ControlMode;
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

    pub fn set_role_enabled(&mut self, role_code: i32, enabled: bool) -> Result<(), JsError> {
        let role = match role_code {
            0 => Role::Server,
            1 => Role::Game,
            2 => Role::Controller,
            _ => return Err(JsError::new("invalid role code")),
        };
        self.inner.set_role_enabled(role, enabled);
        Ok(())
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

    pub fn make_message_invoke(
        &mut self,
        target: &str,
        method: &str,
        return_method: Option<String>,
        params: JsValue,
    ) -> Result<JsValue, JsError> {
        let rust_params: Vec<crate::codec::messages::bm_encoding::Value> =
            if params.is_undefined() || params.is_null() {
                Vec::new()
            } else {
                let arr = js_sys::Array::from(&params);
                let mut out = Vec::new();
                for i in 0..arr.length() {
                    let item = arr.get(i);
                    out.push(js_to_value(item)?);
                }
                out
            };
        let outgoings =
            self.inner
                .make_message_invoke(target, method, return_method.as_deref(), rust_params);
        to_js(&outgoings)
    }

    pub fn make_registry_relay(
        &mut self,
        target: &str,
        dest_slot: i16,
        dest_app: &str,
        dest_id: &str,
        dest_name: &str,
        dest_type: i32,
        dest_addr: &str,
        dest_u_port: i32,
        dest_r_port: i32,
        inner_method: &str,
        inner_return_method: Option<String>,
        inner_params: JsValue,
    ) -> Result<JsValue, JsError> {
        let dt = DeviceType::for_value(dest_type).map_err(|e| JsError::new(&e.to_string()))?;
        let mut core = DeviceCore::new(dest_id.to_string(), dest_name.to_string(), dt);
        let addr = BMAddress {
            address: dest_addr.to_string(),
            unreliable_port: dest_u_port,
            reliable_port: dest_r_port,
        };
        core.address = Some(addr.clone());
        let dest_info = BMRegistryInfo {
            slot_id: dest_slot,
            app_id: dest_app.to_string(),
            current_players: None,
            max_players: None,
            device: core,
            device_address: addr,
        };

        let rust_params: Vec<crate::codec::messages::bm_encoding::Value> =
            if inner_params.is_undefined() || inner_params.is_null() {
                Vec::new()
            } else {
                let arr = js_sys::Array::from(&inner_params);
                let mut out = Vec::new();
                for i in 0..arr.length() {
                    out.push(js_to_value(arr.get(i))?);
                }
                out
            };

        let inner = crate::codec::messages::bm_invoke::BMInvoke {
            id: 0,
            method: inner_method.to_string(),
            return_method: inner_return_method,
            params: rust_params,
        };

        let outgoings = self.inner.make_registry_relay(target, dest_info, inner);
        to_js(&outgoings)
    }

    pub fn make_packet(
        &mut self,
        target: &str,
        channel: i32,
        reliability: i32,
        packet_type_code: i32,
        message: Option<Vec<u8>>,
    ) -> Result<JsValue, JsError> {
        let rel = if reliability < 0 {
            None
        } else {
            Some(reliability)
        };
        let pt = crate::types::packet_type::PacketType::from_i32(packet_type_code)
            .ok_or_else(|| JsError::new("invalid packet_type_code"))?;
        let outgoings = self.inner.make_packet(target, channel, rel, pt, message);
        to_js(&outgoings)
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

    pub fn process_incoming_udp(&mut self, data: &[u8]) -> Result<JsValue, JsError> {
        let result = catch_unwind(AssertUnwindSafe(|| self.inner.process_incoming_udp(data)));

        match result {
            Ok(out) => to_js(&out),
            Err(_) => {
                log::error!("panic in process_incoming_udp");
                Err(JsError::new("Rust panic in process_incoming_udp"))
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

    pub fn make_registry_list(&mut self, target: &str) -> Result<JsValue, JsError> {
        let outgoings = self.inner.make_registry_list(target, None);
        to_js(&outgoings)
    }

    pub fn make_registry_register(
        &mut self,
        target: &str,
        slot_id: i16,
        app_id: &str,
        current_players: i32,
        max_players: i32,
        device_id: &str,
        device_name: &str,
        device_type: i32,
        address: &str,
        unreliable_port: i32,
        reliable_port: i32,
        domain: Option<String>,
    ) -> Result<JsValue, JsError> {
        let dt = DeviceType::for_value(device_type).map_err(|e| JsError::new(&e.to_string()))?;
        let mut core = DeviceCore::new(device_id.to_string(), device_name.to_string(), dt);
        let addr = BMAddress {
            address: address.to_string(),
            unreliable_port,
            reliable_port,
        };
        core.address = Some(addr.clone());

        let info = BMRegistryInfo {
            slot_id,
            app_id: app_id.to_string(),
            current_players: Some(current_players as i16),
            max_players: Some(max_players as i16),
            device: core,
            device_address: addr,
        };

        let outgoings = self
            .inner
            .make_registry_register(target, info, domain, None);
        to_js(&outgoings)
    }

    pub fn make_device_connect_requested(
        &mut self,
        target: &str,
        g_slot: i16,
        g_app: &str,
        g_id: &str,
        g_name: &str,
        g_type: i32,
        g_addr: &str,
        g_u_port: i32,
        g_r_port: i32,
        c_slot: i16,
        c_app: &str,
        c_id: &str,
        c_name: &str,
        c_type: i32,
        c_addr: &str,
        c_u_port: i32,
        c_r_port: i32,
    ) -> Result<JsValue, JsError> {
        let build_info = |slot,
                          app: &str,
                          id: &str,
                          name: &str,
                          type_i,
                          addr_s: &str,
                          u_port,
                          r_port|
         -> Result<BMRegistryInfo, JsError> {
            let dt = DeviceType::for_value(type_i).map_err(|e| JsError::new(&e.to_string()))?;
            let mut core = DeviceCore::new(id.to_string(), name.to_string(), dt);
            let addr = BMAddress {
                address: addr_s.to_string(),
                unreliable_port: u_port,
                reliable_port: r_port,
            };
            core.address = Some(addr.clone());
            Ok(BMRegistryInfo {
                slot_id: slot,
                app_id: app.to_string(),
                current_players: None,
                max_players: None,
                device: core,
                device_address: addr,
            })
        };

        let g_info = build_info(
            g_slot, g_app, g_id, g_name, g_type, g_addr, g_u_port, g_r_port,
        )?;
        let c_info = build_info(
            c_slot, c_app, c_id, c_name, c_type, c_addr, c_u_port, c_r_port,
        )?;

        let outgoings = self
            .inner
            .make_device_connect_requested(target, g_info, c_info);
        to_js(&outgoings)
    }

    pub fn make_set_capabilities(&mut self, target: &str, caps: i32) -> Result<JsValue, JsError> {
        let outgoings = self.inner.make_set_capabilities(target, caps as u64);
        to_js(&outgoings)
    }

    pub fn make_request_xml(
        &mut self,
        target: &str,
        width: i32,
        height: i32,
        device_id: &str,
    ) -> Result<JsValue, JsError> {
        let outgoings = self
            .inner
            .make_request_xml(target, width, height, device_id);
        to_js(&outgoings)
    }

    pub fn make_on_control_scheme_parsed(
        &mut self,
        target: &str,
        device_id: &str,
    ) -> Result<JsValue, JsError> {
        let outgoings = self.inner.make_on_control_scheme_parsed(target, device_id);
        to_js(&outgoings)
    }

    pub fn make_accel(&mut self, target: &str, x: f64, y: f64, z: f64) -> Result<JsValue, JsError> {
        let rel = self
            .inner
            .reliability_for(target, ChannelType::Acceleration.value());
        let outgoings = self.inner.make_accel(target, x, y, z, rel);
        to_js(&outgoings)
    }

    pub fn make_gyro(&mut self, target: &str, x: f32, y: f32, z: f32) -> Result<JsValue, JsError> {
        let rel = self
            .inner
            .reliability_for(target, ChannelType::Gyro.value());
        let outgoings = self.inner.make_gyro(target, x, y, z, rel);
        to_js(&outgoings)
    }

    pub fn make_orientation(
        &mut self,
        target: &str,
        x: f32,
        y: f32,
        z: f32,
        w: f32,
    ) -> Result<JsValue, JsError> {
        let rel = self
            .inner
            .reliability_for(target, ChannelType::Orientation.value());
        let outgoings = self.inner.make_orientation(target, x, y, z, w, rel);
        to_js(&outgoings)
    }

    pub fn make_button_invoke(
        &mut self,
        target: &str,
        handler: &str,
        pressed: bool,
    ) -> Result<JsValue, JsError> {
        let outgoings = self.inner.make_button_invoke(target, handler, pressed);
        to_js(&outgoings)
    }

    pub fn make_dpad_update(&mut self, target: &str, x: i16, y: i16) -> Result<JsValue, JsError> {
        let outgoings = self.inner.make_dpad_update(target, x, y);
        to_js(&outgoings)
    }

    pub fn make_touch_set(&mut self, target: &str, points: JsValue) -> Result<JsValue, JsError> {
        let touches: Vec<Touch> =
            serde_wasm_bindgen::from_value(points).map_err(|e| JsError::new(&e.to_string()))?;
        let rel = self
            .inner
            .reliability_for(target, ChannelType::Touch.value());
        let outgoings = self.inner.make_touch_set(target, touches, rel);
        to_js(&outgoings)
    }

    pub fn make_simple_invoke(
        &mut self,
        target: &str,
        method: &str,
        return_val: Option<String>,
        param: Option<String>,
    ) -> Result<JsValue, JsError> {
        let outgoings = self.inner.make_simple_invoke_string(
            target,
            method,
            return_val.as_deref(),
            param.as_deref(),
        );
        to_js(&outgoings)
    }

    pub fn make_enable_accelerometer(
        &mut self,
        target: &str,
        enabled: bool,
        interval: Option<f64>,
    ) -> Result<JsValue, JsError> {
        let outgoings = self
            .inner
            .make_enable_accelerometer(target, enabled, interval);
        to_js(&outgoings)
    }

    pub fn make_enable_touch(&mut self, target: &str, enabled: bool) -> Result<JsValue, JsError> {
        let outgoings = self.inner.make_enable_touch(target, enabled);
        to_js(&outgoings)
    }

    pub fn make_set_touch_interval(
        &mut self,
        target: &str,
        interval: f64,
    ) -> Result<JsValue, JsError> {
        let outgoings = self.inner.make_set_touch_interval(target, interval);
        to_js(&outgoings)
    }

    pub fn make_enable_gyro(&mut self, target: &str, enabled: bool) -> Result<JsValue, JsError> {
        let outgoings = self.inner.make_enable_gyro(target, enabled);
        to_js(&outgoings)
    }

    pub fn make_set_gyro_interval(
        &mut self,
        target: &str,
        interval: f64,
    ) -> Result<JsValue, JsError> {
        let outgoings = self.inner.make_set_gyro_interval(target, interval);
        to_js(&outgoings)
    }

    pub fn make_enable_orientation(
        &mut self,
        target: &str,
        enabled: bool,
    ) -> Result<JsValue, JsError> {
        let outgoings = self.inner.make_enable_orientation(target, enabled);
        to_js(&outgoings)
    }

    pub fn make_set_orientation_interval(
        &mut self,
        target: &str,
        interval: f64,
    ) -> Result<JsValue, JsError> {
        let outgoings = self.inner.make_set_orientation_interval(target, interval);
        to_js(&outgoings)
    }

    pub fn make_set_reliability_for_touch(
        &mut self,
        target: &str,
        touch_rel: i32,
        control_rel: i32,
    ) -> Result<JsValue, JsError> {
        let outgoings = self
            .inner
            .make_set_reliability_for_touch(target, touch_rel, control_rel);
        to_js(&outgoings)
    }

    pub fn make_set_control_mode(
        &mut self,
        target: &str,
        mode: JsValue,
        text: Option<String>,
    ) -> Result<JsValue, JsError> {
        let mode: ControlMode =
            serde_wasm_bindgen::from_value(mode).map_err(|e| JsError::new(&e.to_string()))?;
        let outgoings = self
            .inner
            .make_set_control_mode(target, mode, text.as_deref());
        to_js(&outgoings)
    }

    pub fn make_wait_for_new_host(
        &mut self,
        target: &str,
        host_id: &str,
    ) -> Result<JsValue, JsError> {
        let outgoings = self.inner.make_wait_for_new_host(target, host_id);
        to_js(&outgoings)
    }

    pub fn make_prompt_trial_upsell(&mut self, target: &str) -> Result<JsValue, JsError> {
        let outgoings = self.inner.make_prompt_trial_upsell(target);
        to_js(&outgoings)
    }

    pub fn make_get_cookie(&mut self, target: &str, name: &str) -> Result<JsValue, JsError> {
        let outgoings = self.inner.make_get_cookie(target, name);
        to_js(&outgoings)
    }

    pub fn make_set_cookie(
        &mut self,
        target: &str,
        name: &str,
        value: &str,
    ) -> Result<JsValue, JsError> {
        let outgoings = self.inner.make_set_cookie(target, name, value);
        to_js(&outgoings)
    }

    pub fn make_update_wallet(&mut self, target: &str) -> Result<JsValue, JsError> {
        let outgoings = self.inner.make_update_wallet(target);
        to_js(&outgoings)
    }

    pub fn make_vibrate(&mut self, target: &str) -> Result<JsValue, JsError> {
        let outgoings = self.inner.make_vibrate(target);
        to_js(&outgoings)
    }
}

fn to_js<T: serde::Serialize>(value: &T) -> Result<JsValue, JsError> {
    serde_wasm_bindgen::to_value(value).map_err(|e| JsError::new(&e.to_string()))
}

fn js_to_value(v: JsValue) -> Result<crate::codec::messages::bm_encoding::Value, JsError> {
    use crate::codec::messages::bm_encoding::Value;
    if let Some(s) = v.as_string() {
        return Ok(Value::String(s));
    }
    if let Some(b) = v.as_bool() {
        return Ok(Value::Bool(b));
    }
    if let Some(f) = v.as_f64() {
        if f.fract() == 0.0 && f >= i32::MIN as f64 && f <= i32::MAX as f64 {
            return Ok(Value::I32(f as i32));
        }
        return Ok(Value::F64(f));
    }
    if js_sys::Array::is_array(&v) {
        let arr = js_sys::Array::from(&v);
        let mut bm_arr = crate::codec::externals::bm_array::BMArray::default();
        for i in 0..arr.length() {
            bm_arr.push(js_to_value(arr.get(i))?);
        }
        return Ok(Value::Object(crate::codec::object::Object::BMArray(bm_arr)));
    }
    Err(JsError::new("unsupported JS value for BM encoding"))
}
