// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use super::Engine;
use crate::codec::Result;
use crate::codec::bm_stream::BMStream;
use crate::codec::externals::bm_packet::BMPacket;
use crate::codec::externals::bm_registry_info::BMRegistryInfo;
use crate::codec::externals::bm_reliability::BMReliability;
use crate::codec::messages::acceleration::Acceleration;
use crate::codec::messages::ack_packet::AckPacket;
use crate::codec::messages::bm_byte_chunk::BMByteChunk;
use crate::codec::messages::bm_encoding::Value;
use crate::codec::messages::bm_gyro::BMGyro;
use crate::codec::messages::bm_invoke::BMInvoke;
use crate::codec::messages::dpad_update::DPadUpdate;
use crate::codec::messages::orientation::Orientation;
use crate::codec::messages::ping::Ping;
use crate::codec::messages::touch::Touch;
use crate::codec::messages::touch_set::TouchSet;
use crate::codec::object::Object;
use crate::devices::device_core::DeviceCore;
use crate::engine::events::{Outgoing, Via};
use crate::engine::methods;
use crate::engine::protocol::serialize_message;
use crate::link::framing::frame;
use crate::types::channel_type::ChannelType;
use crate::types::control_mode::ControlMode;
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
            out.extend(self.make_object_packet(
                target,
                ChannelType::Bytes,
                BMReliability::ReliableUnordered.code(),
                PacketType::Data,
                Object::BMByteChunk(chunk),
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
        let mut out = BMStream::new();
        obj.encode_with_marker(&mut out)?;
        Ok(out.into_inner())
    }

    pub(super) fn make_object_packet(
        &mut self,
        target: &str,
        channel: ChannelType,
        reliability: impl Into<Option<i32>>,
        pkt_type: PacketType,
        obj: Object,
    ) -> Vec<Outgoing> {
        match self.build_object_bytes(obj) {
            Ok(msg) => self.make_packet(
                target,
                channel.value(),
                reliability.into(),
                pkt_type,
                Some(msg),
            ),
            Err(e) => {
                log::error!("build object packet failed: {e}");
                Vec::new()
            }
        }
    }

    pub(super) fn make_invoke_packet(
        &mut self,
        target: &str,
        method: &str,
        return_method: Option<&str>,
        params: Vec<Value>,
    ) -> Vec<Outgoing> {
        match self.build_invoke_payload(method, return_method, params) {
            Ok(msg) => self.make_packet(
                target,
                ChannelType::Message.value(),
                Some(BMReliability::ReliableUnordered.code()),
                PacketType::Data,
                Some(msg),
            ),
            Err(e) => {
                log::error!("build invoke '{method}' failed: {e}");
                Vec::new()
            }
        }
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
        self.make_object_packet(
            target,
            ChannelType::DPad,
            BMReliability::ReliableUnordered.code(),
            PacketType::Data,
            Object::DPadUpdate(DPadUpdate::new(x, y)),
        )
    }

    pub fn make_touch_set(
        &mut self,
        target: &str,
        touches: Vec<Touch>,
        reliability: i32,
    ) -> Vec<Outgoing> {
        self.make_object_packet(
            target,
            ChannelType::Touch,
            reliability,
            PacketType::Data,
            Object::TouchSet(TouchSet { touches }),
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
        self.make_object_packet(
            target,
            ChannelType::Acceleration,
            reliability,
            PacketType::Data,
            Object::Acceleration(Acceleration::new(x, y, z)),
        )
    }

    /// Asks a game what a controller should return to once it is done. A game
    /// that stands on its own answers with nothing.
    pub fn make_get_portal_id(&mut self, target: &str) -> Vec<Outgoing> {
        self.make_message_invoke(
            target,
            methods::GET_PORTAL_ID,
            Some(methods::ON_PORTAL_ID),
            Vec::new(),
        )
    }

    /// Everything a controller says as a session opens, in the order a game
    /// needs to hear it. Capabilities come before the scheme request because a
    /// game can choose what to send from them, so a game asked first would answer
    /// from stale knowledge.
    pub fn make_session_opening(&mut self, target: &str) -> Vec<Outgoing> {
        let session = self.controller_policy.session;
        let mut out = self.make_get_portal_id(target);

        let mask = (session.gyroscope as u64) | ((session.orientation as u64) << 1);
        out.extend(self.make_set_capabilities(target, mask));

        if let Some(viewport) = session.viewport {
            let requester = self
                .state
                .local_device
                .as_ref()
                .map(|d| d.device_id.clone())
                .unwrap_or_default();
            out.extend(self.make_request_xml(target, viewport.width, viewport.height, &requester));
        } else {
            log::warn!(
                "session opening for '{target}': no viewport configured, not requesting a scheme"
            );
        }
        out
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
        self.make_object_packet(
            target,
            ChannelType::Gyro,
            reliability,
            PacketType::Data,
            Object::BMGyro(BMGyro::new(x, y, z)),
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
        self.make_object_packet(
            target,
            ChannelType::Orientation,
            reliability,
            PacketType::Data,
            Object::Orientation(Orientation::new(x, y, z, w)),
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
        mode: ControlMode,
        text_content: Option<&str>,
    ) -> Vec<Outgoing> {
        let mut params = vec![Value::I32(mode.to_wire())];
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
        self.state.local_info = Some(info.clone());

        let mut params = vec![Value::Object(Object::BMRegistryInfo(info))];
        if let Some(d) = domain {
            params.push(Value::String(d));
        }
        let return_method = Self::return_method_or(return_method, methods::DEFAULT_RETURN_REGISTER);
        self.bind_continuation(return_method, Self::rpc_on_register_reply);
        self.make_invoke_packet(
            target,
            methods::REGISTRY_REGISTER,
            Some(return_method),
            params,
        )
    }

    pub fn make_registry_list(
        &mut self,
        target: &str,
        return_method: Option<&str>,
    ) -> Vec<Outgoing> {
        let return_method = Self::return_method_or(return_method, methods::DEFAULT_RETURN_LIST);
        self.bind_continuation(return_method, Self::rpc_on_list);
        self.make_invoke_packet(
            target,
            methods::REGISTRY_LIST,
            Some(return_method),
            Vec::new(),
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
        self.make_invoke_packet(target, methods::REGISTRY_RELAY, Some(""), params)
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

    pub fn make_connection_failed(
        &mut self,
        target: &str,
        controller_info: BMRegistryInfo,
    ) -> Vec<Outgoing> {
        let inner = BMInvoke {
            id: 0,
            method: methods::CONNECTION_FAILED.to_string(),
            return_method: None,
            params: vec![Value::String(self.local_device_id())],
        };
        self.make_registry_relay(target, controller_info, inner)
    }

    pub fn make_message_invoke(
        &mut self,
        target: &str,
        method: &str,
        return_method: Option<&str>,
        params: Vec<Value>,
    ) -> Vec<Outgoing> {
        self.make_invoke_packet(target, method, return_method, params)
    }

    pub fn make_message_invoke_oneway(
        &mut self,
        target: &str,
        method: &str,
        params: Vec<Value>,
    ) -> Vec<Outgoing> {
        self.make_message_invoke(target, method, None, params)
    }

    pub fn make_ping_packet(&mut self, target: &str) -> Vec<Outgoing> {
        let Some(local) = self.state.local_device.clone() else {
            log::warn!("make_ping_packet: no local device");
            return Vec::new();
        };
        let address = local.address.clone().unwrap_or_default();
        let ping = Ping {
            device_id: local.device_id,
            address,
        };
        self.make_object_packet(
            target,
            ChannelType::Broadcast,
            BMReliability::Unreliable.code(),
            PacketType::Ping,
            Object::Ping(ping),
        )
    }

    pub fn make_ack_packet(&mut self, target: &str) -> Vec<Outgoing> {
        let Some(local) = self.state.local_device.clone() else {
            log::warn!("make_ack_packet: no local device");
            return Vec::new();
        };
        let Some(peer) = self.state.registry.get(target).map(|r| r.core.clone()) else {
            log::warn!("make_ack_packet: unknown target {target}");
            return Vec::new();
        };
        let ack = AckPacket::new(peer, local.address.clone().unwrap_or_default());
        self.make_object_packet(
            target,
            ChannelType::Message,
            BMReliability::ReliableUnordered.code(),
            PacketType::Ack,
            Object::AckPacket(ack),
        )
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
        if self.state.registry.get(target).is_none() {
            log::warn!("unknown target device: {target}");
            return Vec::new();
        }

        let rel = reliability.unwrap_or_else(|| Self::default_reliability_for_channel(channel));
        let seq = self.state.next_sequence(channel);

        #[cfg(target_arch = "wasm32")]
        let timestamp_ms = js_sys::Date::now();

        #[cfg(not(target_arch = "wasm32"))]
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as f64;

        let built = {
            let Some(sender) = self
                .state
                .local_device
                .as_ref()
                .or_else(|| self.state.registry.get(target).map(|r| &r.core))
            else {
                return Vec::new();
            };
            self.build_packet_bytes(
                sender,
                channel,
                seq,
                timestamp_ms,
                packet_type.code(),
                message,
            )
        };
        match built {
            Ok(bytes) => vec![self.dispatch(target.to_string(), channel, rel, bytes)],
            Err(e) => {
                log::error!("packet build failed: {e}");
                Vec::new()
            }
        }
    }

    /// Answers a ping with what the ping carried.
    ///
    /// An echo returns the sequence, moment and message it was given, and goes
    /// back the way it came.
    pub(crate) fn make_echo(
        &mut self,
        target: &str,
        ping: &BMPacket,
        datagram: bool,
    ) -> Vec<Outgoing> {
        let Some(rec) = self.state.registry.get(target).cloned() else {
            log::warn!("cannot echo to unknown device: {target}");
            return Vec::new();
        };
        let reliability = if datagram {
            BMReliability::Unreliable.code()
        } else {
            Self::default_reliability_for_channel(ping.channel)
        };

        let sender = self.state.local_device.as_ref().unwrap_or(&rec.core);
        match self.build_packet_bytes(
            sender,
            ping.channel,
            ping.sequence,
            ping.timestamp,
            PacketType::Echo.code(),
            ping.message.clone(),
        ) {
            Ok(bytes) => vec![self.dispatch(target.to_string(), ping.channel, reliability, bytes)],
            Err(e) => {
                log::error!("echo build failed: {e}");
                Vec::new()
            }
        }
    }

    /// Shapes a message for the path it takes to this peer.
    ///
    /// A datagram is chosen when the message goes unreliably and the caller
    /// declared an unreliable path. Everything else is framed and goes over the
    /// stream, which every peer can accept at any time.
    pub(crate) fn dispatch(
        &mut self,
        target: String,
        channel: i32,
        reliability: i32,
        message: Vec<u8>,
    ) -> Outgoing {
        let unreliable = reliability == BMReliability::Unreliable.code();
        let datagram = unreliable
            .then(|| self.datagram_endpoint_of(&target))
            .flatten();

        if unreliable {
            self.note_input_path(&target, datagram.is_some());
        }

        let (via, payload) = match datagram {
            Some((address, port)) => (Via::Datagram { address, port }, message),
            None => (Via::Stream, frame(&message)),
        };
        Outgoing {
            target_device_id: target,
            channel,
            reliability,
            via,
            payload,
        }
    }

    /// Reports the path input takes to a peer, once, and again if it changes.
    fn note_input_path(&mut self, target: &str, datagram: bool) {
        if self.input_paths.insert(target.to_string(), datagram) == Some(datagram) {
            return;
        }
        match self.datagram_endpoint_of(target) {
            Some((address, port)) => {
                log::info!("input to '{target}' goes by datagram, to {address}:{port}")
            }
            None => {
                log::info!("input to '{target}' goes by stream: no unreliable path was declared")
            }
        }
    }

    /// Where a peer takes datagrams, when the caller has an unreliable path to
    /// write to.
    ///
    /// Whether a message wants a datagram is the protocol's call, made through
    /// reliability, and a game that cannot take one says so with
    /// setReliabilityForTouch. So this reports what is known of reaching the
    /// peer rather than judging it: an incomplete answer is a fault to surface,
    /// not a reason to quietly send everything the slow way.
    fn datagram_endpoint_of(&self, target: &str) -> Option<(String, i32)> {
        if !self.datagrams {
            return None;
        }
        let known = self
            .state
            .registry
            .get(target)
            .and_then(|r| r.core.address.clone())
            .unwrap_or_default();
        Some((known.address, known.unreliable_port))
    }

    fn build_packet_bytes(
        &self,
        sender: &DeviceCore,
        channel: i32,
        sequence: i32,
        timestamp_ms: f64,
        packet_type: i32,
        message: Option<Vec<u8>>,
    ) -> std::result::Result<Vec<u8>, String> {
        let pkt = BMPacket {
            sequence,
            channel,
            timestamp: timestamp_ms,
            packet_type: PacketType::from_i32(packet_type).unwrap_or(PacketType::Data),
            device_type: sender.device_type,
            device_name: sender.device_name.clone(),
            device_id: sender.device_id.clone(),
            message,
            ..Default::default()
        };
        serialize_message(&pkt).map_err(|e| e.to_string())
    }

    pub(super) fn default_reliability_for_channel(channel: i32) -> i32 {
        if let Some(ct) = ChannelType::from_i32(channel) {
            match ct {
                ChannelType::Acceleration
                | ChannelType::Touch
                | ChannelType::Gyro
                | ChannelType::Orientation => BMReliability::Unreliable.code(),
                _ => BMReliability::ReliableUnordered.code(),
            }
        } else {
            BMReliability::ReliableUnordered.code()
        }
    }
}

#[cfg(test)]
mod session_tests {
    use crate::config::EngineConfig;
    use crate::devices::device_core::DeviceCore;
    use crate::engine::device_registry::DeviceRecord;
    use crate::engine::events::Arrival;
    use crate::engine::methods;
    use crate::engine::processing::Engine;
    use crate::engine::protocol::deserialize_message;
    use crate::policy::EndpointMode;
    use crate::types::device_type::DeviceType;

    fn controller_with_game(game: &str) -> Engine {
        let mut eng = Engine::default();
        eng.init_local_device(DeviceCore::new(
            "local".to_string(),
            "Local".to_string(),
            DeviceType::Android,
        ));
        eng.configure(EngineConfig {
            endpoint: Some(EndpointMode::Controller),
            opens_sessions: false,
            ..Default::default()
        })
        .unwrap();
        eng.push_registry_update(DeviceRecord::new(
            DeviceCore::new(game.to_string(), "Game".to_string(), DeviceType::Unity),
            None,
        ));
        eng
    }

    fn methods_of(outgoings: &[crate::engine::events::Outgoing]) -> Vec<String> {
        outgoings
            .iter()
            .filter_map(|o| {
                let mut pkt = crate::codec::externals::bm_packet::BMPacket::default();
                deserialize_message(o.message(), &mut pkt).ok()?;
                let msg = pkt.message?;
                let mut cur = crate::codec::bm_stream::BMStream::view(msg.as_slice());
                match crate::codec::object::Object::decode(&mut cur).ok()? {
                    crate::codec::object::Object::BMInvoke(inv) => Some(inv.method),
                    _ => None,
                }
            })
            .collect()
    }

    #[test]
    fn a_session_opens_with_capabilities_before_the_scheme_request() {
        let mut eng = controller_with_game("game1");
        eng.configure(EngineConfig {
            endpoint: Some(EndpointMode::Controller),
            opens_sessions: true,
            gyroscope: true,
            screen_width: 1080,
            screen_height: 2151,
            ..Default::default()
        })
        .unwrap();
        let out = eng.make_session_opening("game1");
        let sent = methods_of(&out);
        assert_eq!(
            sent,
            [
                methods::GET_PORTAL_ID,
                methods::SET_CAPABILITIES,
                methods::REQUEST_XML
            ]
        );
    }

    /// What an invoke asks for, without the packet around it. A packet carries
    /// the moment it was built, so two of them are never byte for byte alike.
    fn invoke_of(outgoing: &crate::engine::events::Outgoing) -> String {
        let mut pkt = crate::codec::externals::bm_packet::BMPacket::default();
        deserialize_message(outgoing.message(), &mut pkt).expect("an outgoing holds a message");
        let msg = pkt.message.expect("and the message holds an object");
        let mut cur = crate::codec::bm_stream::BMStream::view(msg.as_slice());
        match crate::codec::object::Object::decode(&mut cur).expect("which decodes") {
            crate::codec::object::Object::BMInvoke(inv) => {
                format!("{} {:?}", inv.method, inv.params)
            }
            other => panic!("expected an invoke, got {other:?}"),
        }
    }

    #[test]
    fn a_screen_is_reported_upright_however_it_was_measured() {
        let mut upright = controller_with_game("game1");
        upright
            .configure(EngineConfig {
                endpoint: Some(EndpointMode::Controller),
                opens_sessions: true,
                gyroscope: false,
                screen_width: 1080,
                screen_height: 2151,
                ..Default::default()
            })
            .unwrap();
        let mut sideways = controller_with_game("game1");
        sideways
            .configure(EngineConfig {
                endpoint: Some(EndpointMode::Controller),
                opens_sessions: true,
                gyroscope: false,
                screen_width: 2151,
                screen_height: 1080,
                ..Default::default()
            })
            .unwrap();

        let a = upright.make_session_opening("game1");
        let b = sideways.make_session_opening("game1");
        assert_eq!(
            invoke_of(a.last().unwrap()),
            invoke_of(b.last().unwrap()),
            "a screen is the same screen whichever way it was measured"
        );
    }

    /// Building a fake ack the way a game sends one, so the automatic path can
    /// be exercised without a socket.
    fn ack_from(game: &str) -> Vec<u8> {
        let mut eng = Engine::default();
        eng.init_local_device(DeviceCore::new(
            game.to_string(),
            "Game".to_string(),
            DeviceType::Unity,
        ));
        eng.configure(EngineConfig {
            endpoint: Some(EndpointMode::Game),
            opens_sessions: false,
            ..Default::default()
        })
        .unwrap();
        eng.push_registry_update(DeviceRecord::new(
            DeviceCore::new(
                "local".to_string(),
                "Local".to_string(),
                DeviceType::Android,
            ),
            None,
        ));
        eng.make_ack_packet("local").remove(0).message().to_vec()
    }

    #[test]
    fn a_configured_controller_opens_the_session_itself() {
        let mut eng = controller_with_game("game1");
        eng.configure(EngineConfig {
            endpoint: Some(EndpointMode::Controller),
            opens_sessions: true,
            gyroscope: true,
            screen_width: 1080,
            screen_height: 2151,
            ..Default::default()
        })
        .unwrap();
        let out = eng.process_incoming(&ack_from("game1"), &Arrival::default());
        assert_eq!(
            methods_of(&out.outgoings),
            [
                methods::GET_PORTAL_ID,
                methods::SET_CAPABILITIES,
                methods::REQUEST_XML
            ]
        );
    }

    #[test]
    fn a_controller_that_asked_for_nothing_is_left_to_open_its_own() {
        let mut eng = controller_with_game("game1");
        let out = eng.process_incoming(&ack_from("game1"), &Arrival::default());
        assert!(
            out.outgoings.is_empty(),
            "the engine spoke for a caller that never asked it to"
        );
    }

    #[test]
    fn a_controller_can_take_its_sessions_back() {
        let mut eng = controller_with_game("game1");
        eng.configure(EngineConfig {
            endpoint: Some(EndpointMode::Controller),
            opens_sessions: true,
            gyroscope: true,
            screen_width: 1080,
            screen_height: 2151,
            ..Default::default()
        })
        .unwrap();
        eng.configure(EngineConfig {
            endpoint: Some(EndpointMode::Controller),
            opens_sessions: false,
            gyroscope: true,
            screen_width: 1080,
            screen_height: 2151,
            ..Default::default()
        })
        .unwrap();
        let out = eng.process_incoming(&ack_from("game1"), &Arrival::default());
        assert!(out.outgoings.is_empty());
        // The values it handed over are still there to send by hand.
        assert_eq!(
            methods_of(&eng.make_session_opening("game1")),
            [
                methods::GET_PORTAL_ID,
                methods::SET_CAPABILITIES,
                methods::REQUEST_XML
            ]
        );
    }

    #[test]
    fn a_session_without_a_screen_asks_for_no_scheme() {
        let mut eng = controller_with_game("game1");
        let out = eng.make_session_opening("game1");
        let sent = methods_of(&out);
        assert_eq!(sent, [methods::GET_PORTAL_ID, methods::SET_CAPABILITIES]);
    }
}
