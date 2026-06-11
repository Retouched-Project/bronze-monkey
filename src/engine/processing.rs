// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::bm_array::BMArray;
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
use crate::engine::events::{Command, ControlConfig, Event, Outgoing, ProcessOutput, Sensor};
use crate::engine::methods;
use crate::engine::protocol::{deserialize_packet, serialize_packet};
use crate::engine::registry::{DeviceRecord, DeviceRegistry};
use crate::engine::state::EngineState;
use crate::policy::server::PendingRegistration;
use crate::types::channel_type::ChannelType;
use crate::types::device_type::DeviceType;
use crate::types::packet_type::PacketType;
use std::collections::HashMap;
use std::io::Cursor;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ReceivedInvoke {
    pub method: String,
    pub return_method: Option<String>,
    pub params: Vec<Value>,
}

type RpcHandler = fn(&mut Engine, &ReceivedInvoke, Option<&str>, i32, &mut ProcessOutput);

#[derive(Debug, Default, Clone)]
pub struct Engine {
    pub(crate) state: EngineState,
    rpc_handlers: HashMap<String, RpcHandler>,
    pub server_policy: crate::policy::ServerPolicy,
}

impl Engine {
    pub fn new() -> Self {
        let mut handlers: HashMap<String, RpcHandler> = HashMap::with_capacity(32);
        handlers.insert(methods::REGISTRY_REGISTER.to_string(), Self::rpc_registry_register);
        handlers.insert(methods::DEFAULT_RETURN_REGISTER.to_string(), Self::rpc_registry_register);
        handlers.insert(methods::REGISTRY_LIST.to_string(), Self::rpc_registry_list);
        handlers.insert(methods::DEFAULT_RETURN_LIST.to_string(), Self::rpc_registry_list);
        handlers.insert(methods::REGISTRY_RELAY.to_string(), Self::rpc_registry_relay);
        handlers.insert(methods::ON_HOST_CONNECTED.to_string(), Self::rpc_on_host_connected);
        handlers.insert(methods::REGISTRY_UPDATE.to_string(), Self::rpc_registry_update);
        handlers.insert(methods::ON_HOST_UPDATE.to_string(), Self::rpc_registry_update);
        handlers.insert(
            methods::ON_HOST_DISCONNECTED.to_string(),
            Self::rpc_on_host_disconnected,
        );
        handlers.insert(
            methods::DEVICE_CONNECT_REQUESTED.to_string(),
            Self::rpc_device_connect_requested,
        );
        handlers.insert(methods::CONNECTION_FAILED.to_string(), Self::rpc_connection_failed);
        handlers.insert(methods::VIBRATE.to_string(), Self::rpc_vibrate);
        handlers.insert(methods::BM_PAUSE.to_string(), Self::rpc_bm_pause);
        handlers.insert(methods::MENU_EVENT.to_string(), Self::rpc_menu_event);
        handlers.insert(methods::ON_KEY_STRING.to_string(), Self::rpc_on_key_string);
        handlers.insert(
            methods::ON_NAVIGATION_STRING.to_string(),
            Self::rpc_on_navigation_string,
        );
        handlers.insert(methods::SET_CAPABILITIES.to_string(), Self::rpc_set_capabilities);
        handlers.insert(methods::REQUEST_XML.to_string(), Self::rpc_request_xml);
        handlers.insert(
            methods::ON_CONTROL_SCHEME_PARSED.to_string(),
            Self::rpc_on_control_scheme_parsed,
        );
        handlers.insert(methods::GET_COOKIE.to_string(), Self::rpc_get_cookie);
        handlers.insert(methods::SET_COOKIE.to_string(), Self::rpc_set_cookie);
        handlers.insert(methods::GOT_COOKIE.to_string(), Self::rpc_got_cookie);

        Self {
            state: EngineState::new(),
            rpc_handlers: handlers,
            server_policy: crate::policy::ServerPolicy::new(),
        }
    }

    pub fn init_local_device(&mut self, core: DeviceCore) {
        self.state.init_local_device(core);
    }

    pub fn approve_registration(&mut self, device_id: &str) -> Vec<Outgoing> {
        let mut out = Vec::new();
        let Some(PendingRegistration {
            mut info,
            target_id,
            return_method,
        }) = self.server_policy.pending_registrations.remove(device_id)
        else {
            return out;
        };

        let is_game = matches!(
            info.device.device_type,
            DeviceType::Flash | DeviceType::Unity | DeviceType::Native
        );
        if is_game {
            let dev_id = info.device.device_id.clone();
            if let Some(existing) = self
                .state
                .registry
                .get(&dev_id)
                .and_then(|r| r.info.as_ref().map(|i| i.slot_id))
            {
                if existing > 0 {
                    self.state.used_slots.remove(&existing);
                }
            }
            info.slot_id = self.state.allocate_slot();
        } else {
            info.slot_id = 0;
        }
        self.state.upsert_registry_info(info.clone());

        if let Some(reply) = Self::reply_method(return_method.as_deref()) {
            out.extend(self.make_message_invoke(&target_id, reply, None, vec![Value::Bool(true)]));
        } else {
            log::warn!(
                "approve_registration for '{target_id}': no return method on record, skipping reply"
            );
        }

        if is_game {
            let info_val = Value::Object(Object::BMRegistryInfo(info.clone()));

            out.extend(self.make_message_invoke(
                &target_id,
                methods::ON_HOST_CONNECTED,
                None,
                vec![info_val.clone()],
            ));

            let viewer_ids: Vec<String> = self
                .state
                .registry
                .snapshot()
                .into_iter()
                .filter_map(|r| r.info)
                .filter(|r| {
                    !matches!(
                        r.device.device_type,
                        DeviceType::Flash
                            | DeviceType::Unity
                            | DeviceType::Native
                            | DeviceType::Server
                    )
                })
                .filter(|r| r.device.device_id != target_id)
                .map(|r| r.device.device_id)
                .collect();
            for vid in viewer_ids {
                out.extend(self.make_message_invoke(
                    &vid,
                    methods::ON_HOST_CONNECTED,
                    None,
                    vec![info_val.clone()],
                ));
            }
        }
        out
    }

    pub fn deny_registration(&mut self, device_id: &str) -> Vec<Outgoing> {
        let mut out = Vec::new();
        let Some(PendingRegistration {
            target_id,
            return_method,
            ..
        }) = self.server_policy.pending_registrations.remove(device_id)
        else {
            return out;
        };
        if let Some(reply) = Self::reply_method(return_method.as_deref()) {
            out.extend(self.make_message_invoke(&target_id, reply, None, vec![Value::Bool(false)]));
        } else {
            log::warn!(
                "deny_registration for '{target_id}': no return method on record, skipping reply"
            );
        }
        out
    }

    fn reply_method(return_method: Option<&str>) -> Option<&str> {
        return_method.filter(|m| !m.is_empty())
    }

    fn return_method_or<'a>(requested: Option<&'a str>, default: &'a str) -> &'a str {
        match requested {
            Some(m) if !m.is_empty() => m,
            _ => default,
        }
    }

    pub fn emit(&mut self, cmd: Command) -> Vec<Outgoing> {
        match cmd {
            Command::Raw {
                target,
                channel,
                reliability,
                payload,
            } => {
                if target.is_empty() {
                    log::warn!("emit Raw: target device id is empty");
                    return Vec::new();
                }
                vec![Outgoing {
                    target_device_id: target,
                    channel,
                    reliability,
                    payload,
                }]
            }
            Command::SendObject {
                target,
                object,
                channel,
                reliability,
            } => {
                let channel = channel.unwrap_or_else(|| Self::default_channel_for_object(&object));
                let reliability =
                    reliability.unwrap_or_else(|| self.reliability_for(&target, channel));
                let msg = match self.build_object_bytes(object) {
                    Ok(m) => m,
                    Err(e) => {
                        log::error!("emit SendObject: encode failed: {e}");
                        return Vec::new();
                    }
                };
                self.make_packet(
                    &target,
                    channel,
                    Some(reliability),
                    PacketType::Data,
                    Some(msg),
                )
            }
            Command::Invoke {
                target,
                method,
                return_method,
                params,
            } => self.make_message_invoke(&target, &method, return_method.as_deref(), params),
            Command::Relay {
                target,
                destination,
                method,
                return_method,
                params,
            } => {
                let inner = BMInvoke {
                    id: 0,
                    method,
                    return_method,
                    params,
                };
                self.make_registry_relay(&target, destination, inner)
            }
            Command::ApproveRegistration { device_id } => self.approve_registration(&device_id),
            Command::DenyRegistration { device_id } => self.deny_registration(&device_id),
            Command::DropDevice { device_id } => self.drop_device(&device_id),
            Command::Register {
                target,
                info,
                domain,
                return_method,
            } => self.make_registry_register(&target, info, domain, return_method.as_deref()),
            Command::RequestHostList {
                target,
                return_method,
            } => self.make_registry_list(&target, return_method.as_deref()),
            Command::UpdateHostInfo {
                target,
                info,
                return_method,
            } => self.make_message_invoke(
                &target,
                methods::REGISTRY_UPDATE,
                Some(Self::return_method_or(return_method.as_deref(), methods::DEFAULT_RETURN_UPDATE)),
                vec![Value::Object(Object::BMRegistryInfo(info))],
            ),
            Command::Unregister {
                target,
                return_method,
            } => {
                let device_id = self.local_device_id();
                self.make_message_invoke(
                    &target,
                    methods::REGISTRY_REMOVE,
                    Some(Self::return_method_or(return_method.as_deref(), methods::DEFAULT_RETURN_REMOVE)),
                    vec![Value::String(device_id)],
                )
            }
            Command::SetHostVisible {
                target,
                visible,
                notify_everyone,
            } => self.make_message_invoke(
                &target,
                methods::REGISTRY_SET_VISIBLE,
                None,
                vec![Value::Bool(visible), Value::Bool(notify_everyone)],
            ),
            Command::ConnectToHost {
                target,
                host,
                self_info,
            } => self.make_device_connect_requested(&target, host, self_info),
            Command::SendTouch { target, touches } => {
                let reliability = self.reliability_for(&target, ChannelType::Touch.value());
                self.make_touch_set(&target, touches, reliability)
            }
            Command::SendAccel { target, x, y, z } => {
                let reliability = self.reliability_for(&target, ChannelType::Acceleration.value());
                self.make_accel(&target, x, y, z, reliability)
            }
            Command::SendGyro { target, x, y, z } => {
                let reliability = self.reliability_for(&target, ChannelType::Gyro.value());
                self.make_gyro(&target, x, y, z, reliability)
            }
            Command::SendOrientation { target, x, y, z, w } => {
                let reliability = self.reliability_for(&target, ChannelType::Orientation.value());
                self.make_orientation(&target, x, y, z, w, reliability)
            }
            Command::SendDPad { target, x, y } => self.make_dpad_update(&target, x, y),
            Command::SendButton {
                target,
                handler,
                pressed,
            } => self.make_button_invoke(&target, &handler, pressed),
            Command::SendMenuEvent { target, event } => {
                self.make_message_invoke(&target, methods::MENU_EVENT, None, vec![Value::String(event)])
            }
            Command::SendKeyString { target, key } => {
                self.make_message_invoke(&target, methods::ON_KEY_STRING, None, vec![Value::String(key)])
            }
            Command::SendNavigation { target, nav } => self.make_message_invoke(
                &target,
                methods::ON_NAVIGATION_STRING,
                None,
                vec![Value::String(nav)],
            ),
            Command::SetCapabilities {
                target,
                gyroscope,
                orientation,
            } => {
                let mask = (gyroscope as u64) | ((orientation as u64) << 1);
                self.make_set_capabilities(&target, mask)
            }
            Command::ConfigureSensor {
                target,
                sensor,
                enabled,
                interval_ms,
            } => self.configure_sensor(&target, sensor, enabled, interval_ms),
            Command::SetReliability {
                target,
                touch,
                sensors,
            } => self.make_set_reliability_for_touch(&target, touch, sensors),
            Command::SetControlMode { target, mode, text } => {
                self.make_set_control_mode(&target, mode, text.as_deref())
            }
            Command::Vibrate { target } => self.make_vibrate(&target),
            Command::Pause { target } => self.make_message_invoke(&target, methods::BM_PAUSE, None, vec![]),
            Command::RequestControlScheme {
                target,
                width,
                height,
            } => {
                let requester = self.local_device_id();
                self.make_request_xml(&target, width, height, &requester)
            }
            Command::ControlSchemeParsed { target } => {
                let device_id = self.local_device_id();
                self.make_on_control_scheme_parsed(&target, &device_id)
            }
            Command::StoreCookie {
                target,
                name,
                value,
            } => self.make_set_cookie(&target, &name, &value),
            Command::RequestCookie { target, name } => self.make_get_cookie(&target, &name),
            Command::SendCookie {
                target,
                name,
                value,
            } => self.make_message_invoke(
                &target,
                methods::GOT_COOKIE,
                None,
                vec![Value::String(name), Value::String(value)],
            ),
        }
    }

    fn local_device_id(&self) -> String {
        self.state
            .local_device
            .as_ref()
            .map(|d| d.device_id.clone())
            .unwrap_or_default()
    }

    fn configure_sensor(
        &mut self,
        target: &str,
        sensor: Sensor,
        enabled: Option<bool>,
        interval_ms: Option<i32>,
    ) -> Vec<Outgoing> {
        let interval_s = interval_ms.map(|ms| ms as f64 / 1000.0);
        let mut out = Vec::new();
        match sensor {
            Sensor::Accel => {
                if enabled.is_some() || interval_s.is_some() {
                    out.extend(self.make_enable_accelerometer(
                        target,
                        enabled.unwrap_or(true),
                        interval_s,
                    ));
                }
            }
            Sensor::Touch => {
                if let Some(enabled) = enabled {
                    out.extend(self.make_enable_touch(target, enabled));
                }
                if let Some(s) = interval_s {
                    out.extend(self.make_set_touch_interval(target, s));
                }
            }
            Sensor::Gyro => {
                if let Some(enabled) = enabled {
                    out.extend(self.make_enable_gyro(target, enabled));
                }
                if let Some(s) = interval_s {
                    out.extend(self.make_set_gyro_interval(target, s));
                }
            }
            Sensor::Orientation => {
                if let Some(enabled) = enabled {
                    out.extend(self.make_enable_orientation(target, enabled));
                }
                if let Some(s) = interval_s {
                    out.extend(self.make_set_orientation_interval(target, s));
                }
            }
        }
        out
    }

    fn default_channel_for_object(object: &Object) -> i32 {
        match object {
            Object::TouchSet(_) => ChannelType::Touch.value(),
            Object::Acceleration(_) => ChannelType::Acceleration.value(),
            Object::BMGyro(_) => ChannelType::Gyro.value(),
            Object::Orientation(_) => ChannelType::Orientation.value(),
            Object::DPadUpdate(_) => ChannelType::DPad.value(),
            Object::BMByteChunk(_) => ChannelType::Bytes.value(),
            Object::StringLiteral(_) => ChannelType::String.value(),
            _ => ChannelType::Message.value(),
        }
    }

    pub fn registry(&self) -> &DeviceRegistry {
        &self.state.registry
    }

    pub fn registry_mut(&mut self) -> &mut DeviceRegistry {
        &mut self.state.registry
    }

    pub fn register_button_handlers<I>(&mut self, handlers: I)
    where
        I: IntoIterator,
        I::Item: Into<String>,
    {
        self.state
            .button_handlers
            .extend(handlers.into_iter().map(Into::into));
    }

    pub fn clear_button_handlers(&mut self) {
        self.state.button_handlers.clear();
    }

    pub fn process_incoming(&mut self, payload: &[u8]) -> ProcessOutput {
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(
            &format!("WASM: Engine::process_incoming len={}", payload.len()).into(),
        );

        let mut out = ProcessOutput::new();
        let payload_safe = payload.to_vec();

        if payload_safe.is_empty() {
            return out;
        }

        if payload_safe.len() == 12 {
            if let Some(handshake) =
                crate::codec::externals::handshake::Handshake::from_bytes(&payload_safe)
            {
                #[cfg(target_arch = "wasm32")]
                web_sys::console::log_1(&"WASM: Handshake detected".into());

                out.events.push(Event::Handshake {
                    current: handshake.current.to_u32(),
                    minimum: handshake.minimum.to_u32(),
                });
                return out;
            }
        }

        use crate::codec::externals::bm_packet::BMPacket;
        let mut pkt = Box::new(BMPacket::default());
        match deserialize_packet(&payload_safe, &mut pkt) {
            Ok(_) => {
                #[cfg(target_arch = "wasm32")]
                web_sys::console::log_1(&"WASM: deserialize success, calling handle".into());
                self.handle_deserialized_packet(&pkt, &mut out);
            }
            Err(e) => {
                #[cfg(target_arch = "wasm32")]
                web_sys::console::log_1(&format!("WASM: deserialize failed: {}", e).into());
                log::warn!("failed to deserialize packet: {}", e);
            }
        }
        out
    }

    pub fn process_incoming_udp(&mut self, raw: &[u8]) -> ProcessOutput {
        let mut framed = Vec::with_capacity(4 + raw.len());
        framed.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        framed.extend_from_slice(raw);
        self.process_incoming(&framed)
    }

    fn handle_deserialized_packet(&mut self, pkt: &BMPacket, out: &mut ProcessOutput) {
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"WASM: handle_deserialized_packet entry".into());

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(
            &format!(
                "WASM: handle_deserialized_packet type={:?}",
                pkt.packet_type
            )
            .into(),
        );

        let sender_id = if let Some(rec) = self.device_record_from_packet(pkt) {
            let id = rec.core.device_id.clone();
            out.events.push(self.push_registry_update(rec));
            Some(id)
        } else {
            None
        };

        let channel = pkt.channel;
        let pkt_type = pkt.packet_type;
        let reliability = pkt.reliability;

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&format!("WASM: dispatching type={:?}", pkt_type).into());

        match pkt_type {
            PacketType::Ping => self.handle_ping(pkt, channel, sender_id, out),
            PacketType::Ack => self.handle_ack(pkt, out),
            PacketType::Data => self.handle_data(pkt, channel, out),
            _ => log::info!(
                "rx packet type {:?} channel {channel} reliability {reliability}",
                pkt_type
            ),
        }
    }

    fn handle_ping(
        &mut self,
        _pkt: &BMPacket,
        channel: i32,
        sender_id: Option<String>,
        out: &mut ProcessOutput,
    ) {
        log::info!("rx ping");

        if let Some(id) = sender_id {
            out.outgoings.extend(self.make_packet(
                &id,
                channel,
                Some(Self::default_reliability_for_channel(channel)),
                PacketType::Echo,
                None,
            ));
        }
    }

    fn handle_ack(&mut self, pkt: &BMPacket, out: &mut ProcessOutput) {
        if let Some(rec) = self.device_record_from_packet(pkt) {
            out.events.push(Event::PeerConnected { record: rec });
        }
        log::info!("rx ack");
    }

    fn handle_data(&mut self, pkt: &BMPacket, channel: i32, out: &mut ProcessOutput) {
        if let Some(msg) = &pkt.message {
            if !msg.is_empty() {
                #[cfg(target_arch = "wasm32")]
                web_sys::console::log_1(&format!("WASM: handle_data msg_len={}", msg.len()).into());

                let mut cur = Cursor::new(msg);
                match Object::decode(&mut cur) {
                    Ok(obj) => {
                        #[cfg(target_arch = "wasm32")]
                        web_sys::console::log_1(
                            &format!("WASM: Object decode success: class_id={}", obj.class_id())
                                .into(),
                        );

                        match obj {
                            Object::BMInvoke(inv) => self.handle_invoke(
                                ReceivedInvoke {
                                    method: inv.method,
                                    return_method: inv.return_method,
                                    params: inv.params,
                                },
                                Some(pkt.device_id.clone()),
                                channel,
                                out,
                            ),
                            Object::BMByteChunk(chunk) => {
                                let device_id = pkt.device_id.clone();
                                self.handle_chunk(device_id, chunk, out);
                            }
                            Object::TouchSet(ts) => out.events.push(Event::Touch {
                                sender: pkt.device_id.clone(),
                                touches: ts.touches.into_values().collect(),
                            }),
                            Object::Acceleration(a) => out.events.push(Event::Accel {
                                sender: pkt.device_id.clone(),
                                x: a.x,
                                y: a.y,
                                z: a.z,
                            }),
                            Object::BMGyro(g) => out.events.push(Event::Gyro {
                                sender: pkt.device_id.clone(),
                                x: g.x,
                                y: g.y,
                                z: g.z,
                            }),
                            Object::Orientation(o) => out.events.push(Event::Orientation {
                                sender: pkt.device_id.clone(),
                                x: o.x,
                                y: o.y,
                                z: o.z,
                                w: o.w,
                            }),
                            Object::DPadUpdate(d) => out.events.push(Event::DPad {
                                sender: pkt.device_id.clone(),
                                x: d.x,
                                y: d.y,
                            }),
                            _ => {
                                log::debug!(
                                    "rx data object {:?} channel={}",
                                    obj.class_id(),
                                    channel
                                );
                            }
                        }
                    }
                    Err(e) => {
                        #[cfg(target_arch = "wasm32")]
                        web_sys::console::log_1(
                            &format!("WASM: Object decode error: {}", e).into(),
                        );

                        let head = if msg.len() >= 5 {
                            format!(
                                "{:02x} {:02x} {:02x} {:02x} {:02x}",
                                msg[0], msg[1], msg[2], msg[3], msg[4]
                            )
                        } else {
                            "too short".into()
                        };
                        log::debug!(
                            "rx data message decode failed len={} channel={} head={} err={}",
                            msg.len(),
                            channel,
                            head,
                            e
                        );
                    }
                }
            }
        }
    }

    fn handle_chunk(&mut self, device_id: String, chunk: BMByteChunk, out: &mut ProcessOutput) {
        let set_id = chunk.set_id.clone();

        let buffer = self
            .state
            .chunk_buffers
            .entry(set_id.clone())
            .or_insert_with(|| vec![0u8; chunk.total_size as usize]);

        if buffer.len() < chunk.total_size as usize {
            buffer.resize(chunk.total_size as usize, 0);
        }

        let start = chunk.start_byte as usize;
        let end = start + chunk.chunk_size as usize;
        if end <= buffer.len() {
            buffer[start..end].copy_from_slice(&chunk.data);
        } else {
            log::error!(
                "Chunk out of bounds: {}..{} (total {})",
                start,
                end,
                buffer.len()
            );
            return;
        }

        let current = end as u32;
        let total = chunk.total_size as u32;

        out.events.push(Event::ChunkProgress {
            device_id: device_id.clone(),
            set_id: set_id.clone(),
            current,
            total,
        });

        if current >= total {
            if let Some(blob) = self.state.chunk_buffers.remove(&set_id) {
                out.events.push(Event::ChunkComplete {
                    device_id,
                    set_id,
                    blob,
                });
            }
        }
    }

    fn handle_invoke(
        &mut self,
        inv: ReceivedInvoke,
        sender_id: Option<String>,
        channel: i32,
        out: &mut ProcessOutput,
    ) {
        let mut claimed = false;

        if let Some(cfg) = self.parse_control_rpc(&inv) {
            if let Some(sender) = sender_id.as_deref() {
                self.track_reliability(sender, &cfg);
            }
            out.events.push(Event::ControlConfig(cfg));
            claimed = true;
        }

        if let Some(handler) = self.rpc_handlers.get(&inv.method).cloned() {
            handler(self, &inv, sender_id.as_deref(), channel, out);
            claimed = true;
        }

        if !claimed && self.state.button_handlers.contains(&inv.method) {
            if let Some(state) = self.param_string(&inv.params, 0) {
                if state == methods::BUTTON_DOWN || state == methods::BUTTON_UP {
                    out.events.push(Event::Button {
                        sender: sender_id.clone().unwrap_or_default(),
                        handler: inv.method.clone(),
                        pressed: state == methods::BUTTON_DOWN,
                    });
                    claimed = true;
                }
            }
        }

        if !claimed {
            out.events.push(Event::Invoke {
                sender: sender_id,
                method: inv.method,
                return_method: inv.return_method,
                params: inv.params,
            });
        }
    }

    fn rpc_registry_register(
        engine: &mut Engine,
        inv: &ReceivedInvoke,
        sender_id: Option<&str>,
        _channel: i32,
        out: &mut ProcessOutput,
    ) {
        let infos = engine.collect_registry_infos(&inv.params);
        let success = inv.params.iter().find_map(|p| {
            if let Value::Bool(b) = engine.unwrap_value(p) {
                Some(*b)
            } else {
                None
            }
        });

        if !engine.state.is_server() {
            if let Some(success) = success {
                out.events.push(Event::RegistrationResult { success });
            }
            return;
        }

        // Server received a `registry.register` request.
        let Some(target_id) = sender_id else {
            log::warn!("registry.register missing sender id");
            return;
        };

        let Some(mut info) = infos.first().cloned() else {
            return;
        };

        if engine.server_policy.auto_approve_registration {
            let is_game = matches!(
                info.device.device_type,
                DeviceType::Flash | DeviceType::Unity | DeviceType::Native
            );
            if is_game {
                let dev_id = info.device.device_id.clone();
                if let Some(existing) = engine
                    .state
                    .registry
                    .get(&dev_id)
                    .and_then(|r| r.info.as_ref().map(|i| i.slot_id))
                {
                    if existing > 0 {
                        engine.state.used_slots.remove(&existing);
                    }
                }
                info.slot_id = engine.state.allocate_slot();
            } else {
                info.slot_id = 0;
            }
            engine.state.upsert_registry_info(info.clone());

            if let Some(reply) = Self::reply_method(inv.return_method.as_deref()) {
                out.outgoings.extend(engine.make_message_invoke(
                    target_id,
                    reply,
                    None,
                    vec![Value::Bool(true)],
                ));
            } else {
                log::warn!(
                    "registry.register from '{target_id}' omitted a return method, skipping reply"
                );
            }
            out.events.push(Event::PeerRegistered {
                info: info.clone(),
                success: true,
            });

            if is_game {
                let info_val = Value::Object(Object::BMRegistryInfo(info.clone()));

                out.outgoings.extend(engine.make_message_invoke(
                    target_id,
                    methods::ON_HOST_CONNECTED,
                    None,
                    vec![info_val.clone()],
                ));

                let viewer_ids: Vec<String> = engine
                    .state
                    .registry
                    .snapshot()
                    .into_iter()
                    .filter_map(|r| r.info)
                    .filter(|r| {
                        !matches!(
                            r.device.device_type,
                            DeviceType::Flash
                                | DeviceType::Unity
                                | DeviceType::Native
                                | DeviceType::Server
                        )
                    })
                    .filter(|r| r.device.device_id != target_id)
                    .map(|r| r.device.device_id)
                    .collect();
                for vid in viewer_ids {
                    out.outgoings.extend(engine.make_message_invoke(
                        &vid,
                        methods::ON_HOST_CONNECTED,
                        None,
                        vec![info_val.clone()],
                    ));
                }
            }
        } else {
            // Manual approval: stash the request (with the caller's return
            // method) until the integrator calls approve/deny_registration.
            engine.server_policy.pending_registrations.insert(
                info.device.device_id.clone(),
                PendingRegistration {
                    info,
                    target_id: target_id.to_string(),
                    return_method: inv.return_method.clone(),
                },
            );
        }
    }

    fn rpc_registry_list(
        engine: &mut Engine,
        inv: &ReceivedInvoke,
        sender_id: Option<&str>,
        _channel: i32,
        out: &mut ProcessOutput,
    ) {
        let infos = engine.collect_registry_infos(&inv.params);

        if !engine.state.is_server() {
            // Received the full host-list snapshot in response to our request.
            out.events.push(Event::HostList { infos });
            return;
        }

        // Server side: answer the list request via the caller's return method.
        let Some(target_id) = sender_id else {
            return;
        };
        let viewer_type = engine
            .state
            .registry
            .get(target_id)
            .and_then(|r| r.info.as_ref())
            .map(|r| r.device.device_type)
            .unwrap_or(DeviceType::Server);

        let Some(reply) = Self::reply_method(inv.return_method.as_deref()) else {
            log::warn!(
                "registry.list from '{target_id}' omitted a return method, not replying with host list"
            );
            return;
        };

        let list_infos = engine.state.registry_infos_for_viewer(viewer_type);
        let mut arr = BMArray::default();
        for r in list_infos {
            arr.push(Value::Object(Object::BMRegistryInfo(r)));
        }
        out.outgoings.extend(engine.make_message_invoke(
            target_id,
            reply,
            None,
            vec![Value::Object(Object::BMArray(arr))],
        ));
    }

    fn rpc_registry_relay(
        engine: &mut Engine,
        inv: &ReceivedInvoke,
        _sender_id: Option<&str>,
        _channel: i32,
        out: &mut ProcessOutput,
    ) {
        for info in engine.collect_registry_infos(&inv.params) {
            out.events.push(Event::DeviceConnectRequested { info });
        }

        if !engine.state.is_server() {
            return;
        }

        let mut target_id = None;
        let mut relayed = None;

        for p in &inv.params {
            match engine.unwrap_value(p) {
                Value::Object(Object::BMRegistryInfo(r)) => {
                    target_id = Some(r.device.device_id.clone());
                }
                Value::Object(Object::BMInvoke(bm_invoke)) => {
                    relayed = Some(bm_invoke.clone());
                }
                _ => {}
            }
        }

        let Some(target_id) = target_id else {
            return;
        };
        let Some(relayed) = relayed else {
            return;
        };

        out.outgoings.extend(engine.make_message_invoke(
            &target_id,
            &relayed.method,
            relayed.return_method.as_deref(),
            relayed.params,
        ));
    }

    fn rpc_on_host_connected(
        engine: &mut Engine,
        inv: &ReceivedInvoke,
        _sender_id: Option<&str>,
        _channel: i32,
        out: &mut ProcessOutput,
    ) {
        for info in engine.collect_registry_infos(&inv.params) {
            out.events.push(Event::HostConnected { info });
        }
    }

    fn rpc_registry_update(
        engine: &mut Engine,
        inv: &ReceivedInvoke,
        _sender_id: Option<&str>,
        _channel: i32,
        out: &mut ProcessOutput,
    ) {
        let infos = engine.collect_registry_infos(&inv.params);
        for info in &infos {
            out.events.push(Event::HostUpdated { info: info.clone() });
        }

        if !engine.state.is_server() {
            return;
        }

        let viewer_ids: Vec<String> = engine
            .state
            .registry
            .snapshot()
            .into_iter()
            .filter_map(|r| r.info)
            .filter(|r| {
                !matches!(
                    r.device.device_type,
                    DeviceType::Flash | DeviceType::Unity | DeviceType::Native
                )
            })
            .map(|r| r.device.device_id)
            .collect();

        for info in infos.into_iter() {
            engine.state.upsert_registry_info(info.clone());
            if !matches!(
                info.device.device_type,
                DeviceType::Flash | DeviceType::Unity | DeviceType::Native
            ) {
                continue;
            }
            let Some(stored) = engine
                .state
                .registry
                .get(&info.device.device_id)
                .and_then(|r| r.info.clone())
            else {
                continue;
            };
            for vid in &viewer_ids {
                out.outgoings.extend(engine.make_message_invoke(
                    vid,
                    methods::ON_HOST_UPDATE,
                    None,
                    vec![Value::Object(Object::BMRegistryInfo(stored.clone()))],
                ));
            }
        }
    }

    fn rpc_on_host_disconnected(
        engine: &mut Engine,
        inv: &ReceivedInvoke,
        _sender_id: Option<&str>,
        _channel: i32,
        out: &mut ProcessOutput,
    ) {
        for info in engine.collect_registry_infos(&inv.params) {
            out.events.push(Event::HostDisconnected { info });
        }
    }

    fn rpc_device_connect_requested(
        engine: &mut Engine,
        inv: &ReceivedInvoke,
        _sender_id: Option<&str>,
        _channel: i32,
        out: &mut ProcessOutput,
    ) {
        for info in engine.collect_registry_infos(&inv.params) {
            out.events.push(Event::DeviceConnectRequested { info });
        }
    }

    fn sender_string(sender_id: Option<&str>) -> String {
        sender_id.unwrap_or_default().to_string()
    }

    fn rpc_connection_failed(
        engine: &mut Engine,
        inv: &ReceivedInvoke,
        _sender_id: Option<&str>,
        _channel: i32,
        out: &mut ProcessOutput,
    ) {
        let device_id = engine.param_string(&inv.params, 0).unwrap_or_default();
        out.events.push(Event::ConnectionFailed { device_id });
    }

    fn rpc_vibrate(
        _engine: &mut Engine,
        _inv: &ReceivedInvoke,
        sender_id: Option<&str>,
        _channel: i32,
        out: &mut ProcessOutput,
    ) {
        out.events.push(Event::Vibrate {
            sender: Self::sender_string(sender_id),
        });
    }

    fn rpc_bm_pause(
        _engine: &mut Engine,
        _inv: &ReceivedInvoke,
        sender_id: Option<&str>,
        _channel: i32,
        out: &mut ProcessOutput,
    ) {
        out.events.push(Event::Pause {
            sender: Self::sender_string(sender_id),
        });
    }

    fn rpc_menu_event(
        engine: &mut Engine,
        inv: &ReceivedInvoke,
        sender_id: Option<&str>,
        _channel: i32,
        out: &mut ProcessOutput,
    ) {
        let event = engine.param_string(&inv.params, 0).unwrap_or_default();
        out.events.push(Event::MenuEvent {
            sender: Self::sender_string(sender_id),
            event,
        });
    }

    fn rpc_on_key_string(
        engine: &mut Engine,
        inv: &ReceivedInvoke,
        sender_id: Option<&str>,
        _channel: i32,
        out: &mut ProcessOutput,
    ) {
        let key = engine.param_string(&inv.params, 0).unwrap_or_default();
        out.events.push(Event::KeyString {
            sender: Self::sender_string(sender_id),
            key,
        });
    }

    fn rpc_on_navigation_string(
        engine: &mut Engine,
        inv: &ReceivedInvoke,
        sender_id: Option<&str>,
        _channel: i32,
        out: &mut ProcessOutput,
    ) {
        let nav = engine.param_string(&inv.params, 0).unwrap_or_default();
        out.events.push(Event::Navigation {
            sender: Self::sender_string(sender_id),
            nav,
        });
    }

    fn rpc_set_capabilities(
        engine: &mut Engine,
        inv: &ReceivedInvoke,
        sender_id: Option<&str>,
        _channel: i32,
        out: &mut ProcessOutput,
    ) {
        let mask = engine.param_i32(&inv.params, 0).unwrap_or(0);
        out.events.push(Event::Capabilities {
            sender: Self::sender_string(sender_id),
            gyroscope: mask & 1 != 0,
            orientation: mask & 2 != 0,
        });
    }

    fn rpc_request_xml(
        engine: &mut Engine,
        inv: &ReceivedInvoke,
        sender_id: Option<&str>,
        _channel: i32,
        out: &mut ProcessOutput,
    ) {
        let height = engine.param_i32(&inv.params, 0).unwrap_or(0);
        let width = engine.param_i32(&inv.params, 1).unwrap_or(0);
        let requester = engine.param_string(&inv.params, 2).unwrap_or_default();
        out.events.push(Event::ControlSchemeRequested {
            sender: Self::sender_string(sender_id),
            width,
            height,
            requester,
        });
    }

    fn rpc_on_control_scheme_parsed(
        engine: &mut Engine,
        inv: &ReceivedInvoke,
        sender_id: Option<&str>,
        _channel: i32,
        out: &mut ProcessOutput,
    ) {
        let device_id = engine.param_string(&inv.params, 0).unwrap_or_default();
        out.events.push(Event::ControlSchemeParsed {
            sender: Self::sender_string(sender_id),
            device_id,
        });
    }

    fn rpc_get_cookie(
        engine: &mut Engine,
        inv: &ReceivedInvoke,
        sender_id: Option<&str>,
        _channel: i32,
        out: &mut ProcessOutput,
    ) {
        let name = engine.param_string(&inv.params, 0).unwrap_or_default();
        out.events.push(Event::CookieRequested {
            sender: Self::sender_string(sender_id),
            name,
        });
    }

    fn rpc_set_cookie(
        engine: &mut Engine,
        inv: &ReceivedInvoke,
        sender_id: Option<&str>,
        _channel: i32,
        out: &mut ProcessOutput,
    ) {
        let name = engine.param_string(&inv.params, 0).unwrap_or_default();
        let value = engine.param_string(&inv.params, 1).unwrap_or_default();
        out.events.push(Event::CookieStored {
            sender: Self::sender_string(sender_id),
            name,
            value,
        });
    }

    fn rpc_got_cookie(
        engine: &mut Engine,
        inv: &ReceivedInvoke,
        sender_id: Option<&str>,
        _channel: i32,
        out: &mut ProcessOutput,
    ) {
        let name = engine.param_string(&inv.params, 0).unwrap_or_default();
        let value = engine.param_string(&inv.params, 1).unwrap_or_default();
        out.events.push(Event::Cookie {
            sender: Self::sender_string(sender_id),
            name,
            value,
        });
    }

    fn unwrap_value<'a>(&self, v: &'a Value) -> &'a Value {
        if let Value::Object(Object::BMParameter(inner)) = v {
            inner.as_ref()
        } else {
            v
        }
    }

    fn collect_registry_infos(&self, params: &[Value]) -> Vec<BMRegistryInfo> {
        let mut out = Vec::new();
        for p in params {
            let val = self.unwrap_value(p);
            match val {
                Value::Object(Object::BMRegistryInfo(r)) => out.push(r.clone()),
                Value::Object(Object::BMArray(arr)) => {
                    for v in arr.items.iter() {
                        let inner_val = self.unwrap_value(v);
                        if let Value::Object(Object::BMRegistryInfo(r)) = inner_val {
                            out.push(r.clone());
                        }
                    }
                }
                _ => {}
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

    fn build_object_bytes(&self, obj: Object) -> Result<Vec<u8>> {
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
        let state = if pressed { methods::BUTTON_DOWN } else { methods::BUTTON_UP };
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
        self.make_message_invoke(target, methods::ENABLE_TOUCH, None, vec![Value::Bool(enabled)])
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
        self.make_message_invoke(target, methods::ENABLE_GYRO, None, vec![Value::Bool(enabled)])
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
        self.bind_continuation(return_method, Self::rpc_registry_register);
        let msg =
            match self.build_invoke_payload(methods::REGISTRY_REGISTER, Some(return_method), params) {
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
        self.bind_continuation(return_method, Self::rpc_registry_list);
        let msg =
            match self.build_invoke_payload(methods::REGISTRY_LIST, Some(return_method), Vec::new()) {
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

    fn bind_continuation(&mut self, return_method: &str, handler: RpcHandler) {
        if return_method.is_empty() || self.rpc_handlers.contains_key(return_method) {
            return;
        }
        self.rpc_handlers.insert(return_method.to_string(), handler);
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

    fn device_record_from_packet(&self, pkt: &BMPacket) -> Option<DeviceRecord> {
        let core = DeviceCore::new(
            pkt.device_id.clone(),
            pkt.device_name.clone(),
            pkt.device_type,
        );
        Some(DeviceRecord::new(core, None, None))
    }

    pub fn push_registry_update(&mut self, mut record: DeviceRecord) -> Event {
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"WASM: push_registry_update start".into());

        if record.info.is_none() {
            #[cfg(target_arch = "wasm32")]
            web_sys::console::log_1(
                &format!("WASM: checking existing for {}", record.device_id()).into(),
            );

            if let Some(existing) = self.state.registry.get(record.device_id()) {
                #[cfg(target_arch = "wasm32")]
                web_sys::console::log_1(&"WASM: found existing check".into());
                record.info = existing.info.clone();
            }
        }

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"WASM: upserting...".into());

        self.state.registry.upsert(record.clone());

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"WASM: upsert done.".into());
        Event::PeerSeen { record }
    }

    pub fn drop_device(&mut self, device_id: &str) -> Vec<Outgoing> {
        let mut out = Vec::new();
        self.state.input_reliability.remove(device_id);
        if let Some(rec) = self.state.registry.remove(device_id) {
            if let Some(info) = rec.info {
                if info.slot_id > 0 {
                    self.state.used_slots.remove(&info.slot_id);
                }

                // If a game disconnected, broadcast onHostDisconnected to all controllers
                // so they can remove it from their host list
                let is_game = matches!(
                    info.device.device_type,
                    DeviceType::Flash | DeviceType::Unity | DeviceType::Native
                );
                if is_game && self.state.is_server() {
                    let info_val = Value::Object(Object::BMRegistryInfo(info));
                    let viewer_ids: Vec<String> = self
                        .state
                        .registry
                        .snapshot()
                        .into_iter()
                        .filter_map(|r| r.info)
                        .filter(|r| {
                            !matches!(
                                r.device.device_type,
                                DeviceType::Flash
                                    | DeviceType::Unity
                                    | DeviceType::Native
                                    | DeviceType::Server
                            )
                        })
                        .map(|r| r.device.device_id)
                        .collect();
                    for vid in viewer_ids {
                        out.extend(self.make_message_invoke(
                            &vid,
                            methods::ON_HOST_DISCONNECTED,
                            None,
                            vec![info_val.clone()],
                        ));
                    }
                }
            }
        }
        out
    }

    fn default_reliability_for_channel(channel: i32) -> i32 {
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

    fn track_reliability(&mut self, sender: &str, cfg: &ControlConfig) {
        if cfg.touch_reliability.is_none() && cfg.control_reliability.is_none() {
            return;
        }
        let entry = self
            .state
            .input_reliability
            .entry(sender.to_string())
            .or_default();
        if let Some(touch) = cfg.touch_reliability {
            entry.touch = Some(touch);
        }
        if let Some(sensors) = cfg.control_reliability {
            entry.sensors = Some(sensors);
        }
    }

    pub fn reliability_for(&self, target: &str, channel: i32) -> i32 {
        let tracked = self.state.input_reliability.get(target);
        let requested = match ChannelType::from_i32(channel) {
            Some(ChannelType::Touch) => tracked.and_then(|r| r.touch),
            Some(ChannelType::Acceleration | ChannelType::Gyro | ChannelType::Orientation) => {
                tracked.and_then(|r| r.sensors)
            }
            _ => None,
        };
        requested.unwrap_or_else(|| Self::default_reliability_for_channel(channel))
    }

    fn parse_control_rpc(&self, inv: &ReceivedInvoke) -> Option<ControlConfig> {
        let mut touch_enabled = None;
        let mut accel_enabled = None;
        let mut gyro_enabled = None;
        let mut orientation_enabled = None;
        let mut touch_interval_ms = None;
        let mut accel_interval_ms = None;
        let mut gyro_interval_ms = None;
        let mut orientation_interval_ms = None;
        let mut touch_reliability = None;
        let mut control_reliability = None;
        let mut control_mode = None;
        let mut portal_id = None;
        let mut return_app_id = None;

        match inv.method.as_str() {
            methods::ENABLE_ACCELEROMETER => {
                touch_enabled = None;
                accel_enabled = self.param_bool(&inv.params, 0);
                if let Some(sec) = self.param_f64(&inv.params, 1) {
                    accel_interval_ms = Some((sec * 1000.0) as i32);
                }
            }
            methods::ENABLE_TOUCH => {
                touch_enabled = self.param_bool(&inv.params, 0);
            }
            methods::SET_TOUCH_INTERVAL => {
                if let Some(sec) = self.param_f64(&inv.params, 0) {
                    touch_interval_ms = Some((sec * 1000.0) as i32);
                }
            }
            methods::ENABLE_GYRO => {
                gyro_enabled = self.param_bool(&inv.params, 0);
            }
            methods::SET_GYRO_INTERVAL => {
                if let Some(sec) = self.param_f64(&inv.params, 0) {
                    gyro_interval_ms = Some((sec * 1000.0) as i32);
                }
            }
            methods::ENABLE_ORIENTATION => {
                orientation_enabled = self.param_bool(&inv.params, 0);
            }
            methods::SET_ORIENTATION_INTERVAL => {
                if let Some(sec) = self.param_f64(&inv.params, 0) {
                    orientation_interval_ms = Some((sec * 1000.0) as i32);
                }
            }
            methods::SET_RELIABILITY_FOR_TOUCH => {
                touch_reliability = self.param_i32(&inv.params, 0);
                control_reliability = self.param_i32(&inv.params, 1);
            }
            methods::SET_CONTROL_MODE => {
                control_mode = self.param_i32(&inv.params, 0);
                return_app_id = self.param_string(&inv.params, 1);
            }
            methods::WAIT_FOR_NEW_HOST => {
                portal_id = self.param_string(&inv.params, 0);
                control_mode = Some(3);
            }
            methods::ON_PORTAL_ID => {
                return_app_id = self.param_string(&inv.params, 0);
            }
            _ => return None,
        }

        Some(ControlConfig {
            touch_enabled,
            accel_enabled,
            gyro_enabled,
            orientation_enabled,
            touch_interval_ms,
            accel_interval_ms,
            gyro_interval_ms,
            orientation_interval_ms,
            touch_reliability,
            control_reliability,
            control_mode,
            portal_id,
            return_app_id,
        })
    }

    fn param_bool(&self, params: &[Value], idx: usize) -> Option<bool> {
        match params.get(idx)? {
            Value::Bool(b) => Some(*b),
            Value::I16(v) => Some(*v != 0),
            Value::I32(v) => Some(*v != 0),
            Value::U16(v) => Some(*v != 0),
            Value::U32(v) => Some(*v != 0),
            _ => None,
        }
    }

    fn param_i32(&self, params: &[Value], idx: usize) -> Option<i32> {
        match params.get(idx)? {
            Value::I16(v) => Some(*v as i32),
            Value::I32(v) => Some(*v),
            Value::U16(v) => Some(*v as i32),
            Value::U32(v) => Some(*v as i32),
            _ => None,
        }
    }

    fn param_f64(&self, params: &[Value], idx: usize) -> Option<f64> {
        match params.get(idx)? {
            Value::F32(v) => Some(*v as f64),
            Value::F64(v) => Some(*v),
            Value::I16(v) => Some(*v as f64),
            Value::I32(v) => Some(*v as f64),
            Value::U16(v) => Some(*v as f64),
            Value::U32(v) => Some(*v as f64),
            _ => None,
        }
    }

    fn param_string(&self, params: &[Value], idx: usize) -> Option<String> {
        match params.get(idx)? {
            Value::String(s) => Some(s.clone()),
            _ => None,
        }
    }
}
