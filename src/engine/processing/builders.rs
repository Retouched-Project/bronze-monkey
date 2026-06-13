// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use super::Engine;
use crate::codec::externals::bm_packet::BMPacket;
use crate::codec::externals::bm_registry_info::BMRegistryInfo;
use crate::codec::externals::bm_reliability::BMReliability;
use crate::codec::io::Result;
use crate::codec::messages::acceleration::Acceleration;
use crate::codec::messages::bm_byte_chunk::BMByteChunk;
use crate::codec::messages::bm_encoding::Value;
use crate::codec::messages::bm_gyro::BMGyro;
use crate::codec::messages::bm_invoke::BMInvoke;
use crate::codec::messages::bm_parameter::VecOutput;
use crate::codec::messages::dpad_update::DPadUpdate;
use crate::codec::messages::orientation::Orientation;
use crate::codec::messages::touch::Touch;
use crate::codec::messages::touch_set::TouchSet;
use crate::codec::object::Object;
use crate::devices::device_core::DeviceCore;
use crate::engine::events::Outgoing;
use crate::engine::methods;
use crate::engine::protocol::serialize_packet;
use crate::types::channel_type::ChannelType;
use crate::types::packet_type::PacketType;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

impl Engine {
    pub fn make_byte_chunks(&mut self, target: &str, set_id: &str, blob: &[u8]) -> Vec<Outgoing> {
        const CHUNK_SIZE: usize = 10240;
        let total_size = blob.len() as i32;
        let mut out = Vec::new();
        let mut start = 0usize;
        loop {
            let len = (blob.len() - start).min(CHUNK_SIZE);
            let chunk = BMByteChunk {
                set_id: set_id.to_string(),
                start_byte: start as i32,
                chunk_size: len as i32,
                total_size,
                data: blob[start..start + len].to_vec(),
            };
            let msg = match self.build_object_bytes(Object::BMByteChunk(chunk)) {
                Ok(m) => m,
                Err(e) => {
                    log::error!("build chunk failed: {e}");
                    return Vec::new();
                }
            };
            out.extend(self.make_packet(
                target,
                ChannelType::Bytes.value(),
                Some(BMReliability::Reliable.code()),
                PacketType::Data,
                Some(msg),
            ));
            start += len;
            if start >= blob.len() {
                break;
            }
        }
        out
    }

    pub fn build_invoke_payload(
        &mut self,
        method: &str,
        return_method: Option<&str>,
        params: Vec<Value>,
    ) -> Result<Vec<u8>> {
        let invoke = BMInvoke {
            id: self.state.next_invoke_id(),
            method: method.to_string(),
            return_method: return_method.map(|s| s.to_string()),
            params,
        };
        self.build_object_bytes(Object::BMInvoke(invoke))
    }

    pub(super) fn build_object_bytes(&self, obj: Object) -> Result<Vec<u8>> {
        let mut out = VecOutput::default();
        obj.encode_with_marker(&mut out)?;
        Ok(out.buf)
    }

    pub fn make_button_invoke(
        &mut self,
        target: &str,
        handler: &str,
        pressed: bool,
    ) -> Vec<Outgoing> {
        let state = if pressed {
            methods::BUTTON_DOWN
        } else {
            methods::BUTTON_UP
        };
        self.make_message_invoke(
            target,
            handler,
            None,
            vec![Value::String(state.to_string())],
        )
    }

    pub fn make_dpad_update(&mut self, target: &str, x: i16, y: i16) -> Vec<Outgoing> {
        let msg = match self.build_object_bytes(Object::DPadUpdate(DPadUpdate::new(x, y))) {
            Ok(m) => m,
            Err(e) => {
                log::error!("build dpad failed: {e}");
                return Vec::new();
            }
        };
        self.make_packet(
            target,
            ChannelType::DPad.value(),
            Some(BMReliability::Reliable.code()),
            PacketType::Data,
            Some(msg),
        )
    }

    pub fn make_touch_set(
        &mut self,
        target: &str,
        touches: Vec<Touch>,
        reliability: i32,
    ) -> Vec<Outgoing> {
        let mut map = std::collections::HashMap::with_capacity(touches.len());
        for t in touches {
            map.insert(t.id, t);
        }
        let touch_set = TouchSet { touches: map };
        let msg = match self.build_object_bytes(Object::TouchSet(touch_set)) {
            Ok(m) => m,
            Err(e) => {
                log::error!("build touch failed: {e}");
                return Vec::new();
            }
        };
        self.make_packet(
            target,
            ChannelType::Touch.value(),
            Some(reliability),
            PacketType::Data,
            Some(msg),
        )
    }

    pub fn make_accel(
        &mut self,
        target: &str,
        x: f64,
        y: f64,
        z: f64,
        reliability: i32,
    ) -> Vec<Outgoing> {
        let msg = match self.build_object_bytes(Object::Acceleration(Acceleration::new(x, y, z))) {
            Ok(m) => m,
            Err(e) => {
                log::error!("build accel failed: {e}");
                return Vec::new();
            }
        };
        self.make_packet(
            target,
            ChannelType::Acceleration.value(),
            Some(reliability),
            PacketType::Data,
            Some(msg),
        )
    }

    pub fn make_request_xml(
        &mut self,
        target: &str,
        width: i32,
        height: i32,
        requester_device_id: &str,
    ) -> Vec<Outgoing> {
        let params = vec![
            Value::I32(height),
            Value::I32(width),
            Value::String(requester_device_id.to_string()),
        ];
        self.make_message_invoke(target, methods::REQUEST_XML, None, params)
    }

    pub fn make_on_control_scheme_parsed(
        &mut self,
        target: &str,
        device_id: &str,
    ) -> Vec<Outgoing> {
        let params = vec![Value::String(device_id.to_string())];
        self.make_message_invoke(target, methods::ON_CONTROL_SCHEME_PARSED, None, params)
    }

    pub fn make_simple_invoke_string(
        &mut self,
        target: &str,
        method: &str,
        return_method: Option<&str>,
        param_str: Option<&str>,
    ) -> Vec<Outgoing> {
        let mut params = Vec::new();
        if let Some(s) = param_str {
            params.push(Value::String(s.to_string()));
        }
        self.make_message_invoke(target, method, return_method, params)
    }

    pub fn make_gyro(
        &mut self,
        target: &str,
        x: f32,
        y: f32,
        z: f32,
        reliability: i32,
    ) -> Vec<Outgoing> {
        let msg = match self.build_object_bytes(Object::BMGyro(BMGyro::new(x, y, z))) {
            Ok(m) => m,
            Err(e) => {
                log::error!("build gyro failed: {e}");
                return Vec::new();
            }
        };
        self.make_packet(
            target,
            ChannelType::Gyro.value(),
            Some(reliability),
            PacketType::Data,
            Some(msg),
        )
    }

    pub fn make_orientation(
        &mut self,
        target: &str,
        x: f32,
        y: f32,
        z: f32,
        w: f32,
        reliability: i32,
    ) -> Vec<Outgoing> {
        let msg = match self.build_object_bytes(Object::Orientation(Orientation::new(x, y, z, w))) {
            Ok(m) => m,
            Err(e) => {
                log::error!("build orientation failed: {e}");
                return Vec::new();
            }
        };
        self.make_packet(
            target,
            ChannelType::Orientation.value(),
            Some(reliability),
            PacketType::Data,
            Some(msg),
        )
    }

    pub fn make_vibrate(&mut self, target: &str) -> Vec<Outgoing> {
        self.make_message_invoke(target, methods::VIBRATE, None, vec![])
    }

    pub fn make_update_wallet(&mut self, target: &str) -> Vec<Outgoing> {
        self.make_message_invoke(target, methods::UPDATE_WALLET, None, vec![])
    }

    pub fn make_get_cookie(&mut self, target: &str, name: &str) -> Vec<Outgoing> {
        self.make_message_invoke(
            target,
            methods::GET_COOKIE,
            None,
            vec![Value::String(name.to_string())],
        )
    }

    pub fn make_set_cookie(&mut self, target: &str, name: &str, value: &str) -> Vec<Outgoing> {
        self.make_message_invoke(
            target,
            methods::SET_COOKIE,
            None,
            vec![
                Value::String(name.to_string()),
                Value::String(value.to_string()),
            ],
        )
    }

    pub fn make_prompt_trial_upsell(&mut self, target: &str) -> Vec<Outgoing> {
        self.make_message_invoke(target, methods::PROMPT_TRIAL_UPSELL, None, vec![])
    }

    pub fn make_wait_for_new_host(&mut self, target: &str, host_device_id: &str) -> Vec<Outgoing> {
        self.make_message_invoke(
            target,
            methods::WAIT_FOR_NEW_HOST,
            None,
            vec![Value::String(host_device_id.to_string())],
        )
    }

    pub fn make_set_control_mode(
        &mut self,
        target: &str,
        mode: i32,
        text_content: Option<&str>,
    ) -> Vec<Outgoing> {
        let mut params = vec![Value::I32(mode)];
        if let Some(text) = text_content {
            params.push(Value::String(text.to_string()));
        }
        self.make_message_invoke(target, methods::SET_CONTROL_MODE, None, params)
    }

    pub fn make_enable_accelerometer(
        &mut self,
        target: &str,
        enabled: bool,
        interval_seconds: Option<f64>,
    ) -> Vec<Outgoing> {
        let mut params = vec![Value::Bool(enabled)];
        if let Some(interval) = interval_seconds {
            params.push(Value::F64(interval));
        }
        self.make_message_invoke(target, methods::ENABLE_ACCELEROMETER, None, params)
    }

    pub fn make_enable_touch(&mut self, target: &str, enabled: bool) -> Vec<Outgoing> {
        self.make_message_invoke(
            target,
            methods::ENABLE_TOUCH,
            None,
            vec![Value::Bool(enabled)],
        )
    }

    pub fn make_set_touch_interval(
        &mut self,
        target: &str,
        interval_seconds: f64,
    ) -> Vec<Outgoing> {
        self.make_message_invoke(
            target,
            methods::SET_TOUCH_INTERVAL,
            None,
            vec![Value::F64(interval_seconds)],
        )
    }

    pub fn make_enable_gyro(&mut self, target: &str, enabled: bool) -> Vec<Outgoing> {
        self.make_message_invoke(
            target,
            methods::ENABLE_GYRO,
            None,
            vec![Value::Bool(enabled)],
        )
    }

    pub fn make_set_gyro_interval(&mut self, target: &str, interval_seconds: f64) -> Vec<Outgoing> {
        self.make_message_invoke(
            target,
            methods::SET_GYRO_INTERVAL,
            None,
            vec![Value::F64(interval_seconds)],
        )
    }

    pub fn make_enable_orientation(&mut self, target: &str, enabled: bool) -> Vec<Outgoing> {
        self.make_message_invoke(
            target,
            methods::ENABLE_ORIENTATION,
            None,
            vec![Value::Bool(enabled)],
        )
    }

    pub fn make_set_orientation_interval(
        &mut self,
        target: &str,
        interval_seconds: f64,
    ) -> Vec<Outgoing> {
        self.make_message_invoke(
            target,
            methods::SET_ORIENTATION_INTERVAL,
            None,
            vec![Value::F64(interval_seconds)],
        )
    }

    pub fn make_set_reliability_for_touch(
        &mut self,
        target: &str,
        touch_reliability: i32,
        control_reliability: i32,
    ) -> Vec<Outgoing> {
        self.make_message_invoke(
            target,
            methods::SET_RELIABILITY_FOR_TOUCH,
            None,
            vec![
                Value::I32(touch_reliability),
                Value::I32(control_reliability),
            ],
        )
    }

    pub fn make_set_capabilities(&mut self, target: &str, capabilities: u64) -> Vec<Outgoing> {
        self.make_message_invoke(
            target,
            methods::SET_CAPABILITIES,
            None,
            vec![Value::U32(capabilities as u32)],
        )
    }

    pub fn make_registry_register(
        &mut self,
        target: &str,
        info: BMRegistryInfo,
        domain: Option<String>,
        return_method: Option<&str>,
    ) -> Vec<Outgoing> {
        let mut params = vec![Value::Object(Object::BMRegistryInfo(info))];
        if let Some(d) = domain {
            params.push(Value::String(d));
        }
        let return_method = Self::return_method_or(return_method, methods::DEFAULT_RETURN_REGISTER);
        self.bind_continuation(return_method, Self::rpc_on_register_reply);
        let msg = match self.build_invoke_payload(
            methods::REGISTRY_REGISTER,
            Some(return_method),
            params,
        ) {
            Ok(m) => m,
            Err(e) => {
                log::error!("build register invoke failed: {e}");
                return Vec::new();
            }
        };
        self.make_packet(
            target,
            ChannelType::Message.value(),
            Some(BMReliability::Reliable.code()),
            PacketType::Data,
            Some(msg),
        )
    }

    pub fn make_registry_list(
        &mut self,
        target: &str,
        return_method: Option<&str>,
    ) -> Vec<Outgoing> {
        let return_method = Self::return_method_or(return_method, methods::DEFAULT_RETURN_LIST);
        self.bind_continuation(return_method, Self::rpc_on_list);
        let msg = match self.build_invoke_payload(
            methods::REGISTRY_LIST,
            Some(return_method),
            Vec::new(),
        ) {
            Ok(m) => m,
            Err(e) => {
                log::error!("build list invoke failed: {e}");
                return Vec::new();
            }
        };
        self.make_packet(
            target,
            ChannelType::Message.value(),
            Some(BMReliability::Reliable.code()),
            PacketType::Data,
            Some(msg),
        )
    }

    pub fn make_registry_relay(
        &mut self,
        target: &str,
        dest_info: BMRegistryInfo,
        inner: BMInvoke,
    ) -> Vec<Outgoing> {
        let inner_obj = Value::Object(Object::BMInvoke(inner));
        let params = vec![Value::Object(Object::BMRegistryInfo(dest_info)), inner_obj];
        let msg = match self.build_invoke_payload(methods::REGISTRY_RELAY, Some(""), params) {
            Ok(m) => m,
            Err(e) => {
                log::error!("build relay invoke failed: {e}");
                return Vec::new();
            }
        };
        self.make_packet(
            target,
            ChannelType::Message.value(),
            Some(BMReliability::Reliable.code()),
            PacketType::Data,
            Some(msg),
        )
    }

    pub fn make_device_connect_requested(
        &mut self,
        target: &str,
        game_info: BMRegistryInfo,
        controller_info: BMRegistryInfo,
    ) -> Vec<Outgoing> {
        let inner = BMInvoke {
            id: 0,
            method: methods::DEVICE_CONNECT_REQUESTED.to_string(),
            return_method: None,
            params: vec![Value::Object(Object::BMRegistryInfo(controller_info))],
        };
        self.make_registry_relay(target, game_info, inner)
    }

    pub fn make_message_invoke(
        &mut self,
        target: &str,
        method: &str,
        return_method: Option<&str>,
        params: Vec<Value>,
    ) -> Vec<Outgoing> {
        let msg = match self.build_invoke_payload(method, return_method, params) {
            Ok(m) => m,
            Err(e) => {
                log::error!("build invoke failed: {e}");
                return Vec::new();
            }
        };
        self.make_packet(
            target,
            ChannelType::Message.value(),
            Some(BMReliability::Reliable.code()),
            PacketType::Data,
            Some(msg),
        )
    }

    pub fn make_message_invoke_oneway(
        &mut self,
        target: &str,
        method: &str,
        params: Vec<Value>,
    ) -> Vec<Outgoing> {
        self.make_message_invoke(target, method, None, params)
    }

    pub fn make_packet(
        &mut self,
        target: &str,
        channel: i32,
        reliability: Option<i32>,
        packet_type: PacketType,
        message: Option<Vec<u8>>,
    ) -> Vec<Outgoing> {
        if target.is_empty() {
            log::warn!("target device id is empty");
            return Vec::new();
        }
        let Some(rec) = self.state.registry.get(target).cloned() else {
            log::warn!("unknown target device: {target}");
            return Vec::new();
        };

        let rel = reliability.unwrap_or_else(|| Self::default_reliability_for_channel(channel));
        let seq = self.state.next_sequence(channel);

        #[cfg(target_arch = "wasm32")]
        let timestamp_ms = js_sys::Date::now();

        #[cfg(not(target_arch = "wasm32"))]
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as f64;

        let sender = self.state.local_device.as_ref().unwrap_or(&rec.core);
        match self.build_packet_bytes(
            sender,
            channel,
            seq,
            timestamp_ms,
            rel,
            packet_type.code(),
            message,
        ) {
            Ok(bytes) => vec![Outgoing {
                target_device_id: target.to_string(),
                channel,
                reliability: rel,
                payload: bytes,
            }],
            Err(e) => {
                log::error!("packet build failed: {e}");
                Vec::new()
            }
        }
    }

    fn build_packet_bytes(
        &self,
        sender: &DeviceCore,
        channel: i32,
        sequence: i32,
        timestamp_ms: f64,
        reliability: i32,
        packet_type: i32,
        message: Option<Vec<u8>>,
    ) -> std::result::Result<Vec<u8>, String> {
        let pkt = BMPacket::new(
            sequence,
            channel,
            timestamp_ms,
            0.0,
            PacketType::from_i32(packet_type).unwrap_or(PacketType::Data),
            sender.device_type,
            reliability,
            sender.device_name.clone(),
            sender.device_id.clone(),
            message,
            None,
            0,
            0,
        );
        serialize_packet(&pkt).map_err(|e| e.to_string())
    }

    pub(super) fn default_reliability_for_channel(channel: i32) -> i32 {
        if let Some(ct) = ChannelType::from_i32(channel) {
            match ct {
                ChannelType::Acceleration
                | ChannelType::Touch
                | ChannelType::Gyro
                | ChannelType::Orientation => BMReliability::Unreliable.code(),
                _ => BMReliability::Reliable.code(),
            }
        } else {
            BMReliability::Reliable.code()
        }
    }
}
