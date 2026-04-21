// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::devices::device_core::DeviceCore;
use crate::engine::actions::{Action, RegistryEventKind};
use crate::engine::protocol::{deserialize_packet, serialize_packet};
use crate::engine::registry::{DeviceRecord, DeviceRegistry};
use crate::codec::externals::bm_array::BMArray;
use crate::codec::externals::bm_packet::BMPacket;
use crate::codec::externals::bm_registry_info::BMRegistryInfo;
use crate::codec::externals::bm_reliability::BMReliability;
use crate::codec::io::Result;
use crate::codec::object::Object;
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
use crate::types::channel_type::ChannelType;
use crate::types::device_type::DeviceType;
use crate::types::packet_type::PacketType;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ReceivedInvoke {
    pub method: String,
    pub return_method: Option<String>,
    pub params: Vec<Value>,
}

type RpcHandler = fn(&mut Engine, &[Value], Option<&str>, i32) -> Vec<Action>;

#[derive(Debug, Default, Clone)]
pub struct Engine {
    registry: DeviceRegistry,
    seq_by_channel: HashMap<i32, i32>,
    local_device: Option<DeviceCore>,
    chunk_buffers: HashMap<String, Vec<u8>>,
    invoke_counter: i32,
    rpc_handlers: HashMap<String, RpcHandler>,
    used_slots: HashSet<i16>,
    pub auto_approve_registration: bool,
    pending_registrations: HashMap<String, (BMRegistryInfo, String)>,
}

impl Engine {
    pub fn new() -> Self {
        let mut handlers: HashMap<String, RpcHandler> = HashMap::with_capacity(16);
        handlers.insert("registry.register".to_string(), Self::rpc_registry_register);
        handlers.insert("onRegister".to_string(), Self::rpc_registry_register);
        handlers.insert("registry.list".to_string(), Self::rpc_registry_list);
        handlers.insert("onList".to_string(), Self::rpc_registry_list);
        handlers.insert("registry.relay".to_string(), Self::rpc_registry_relay);
        handlers.insert("onHostConnected".to_string(), Self::rpc_on_host_connected);
        handlers.insert("registry.update".to_string(), Self::rpc_registry_update);
        handlers.insert("onHostUpdate".to_string(), Self::rpc_registry_update);
        handlers.insert(
            "onHostDisconnected".to_string(),
            Self::rpc_on_host_disconnected,
        );
        handlers.insert(
            "deviceConnectRequested".to_string(),
            Self::rpc_device_connect_requested,
        );

        Self {
            registry: DeviceRegistry::default(),
            seq_by_channel: HashMap::new(),
            local_device: None,
            chunk_buffers: HashMap::new(),
            invoke_counter: 1,
            rpc_handlers: handlers,
            used_slots: HashSet::new(),
            auto_approve_registration: true,
            pending_registrations: HashMap::new(),
        }
    }

    fn is_server(&self) -> bool {
        self.local_device
            .as_ref()
            .map(|d| d.device_type == DeviceType::Server)
            .unwrap_or(true)
    }

    pub fn init_local_device(&mut self, core: DeviceCore) {
        self.local_device = Some(core);
    }

    pub fn approve_registration(&mut self, device_id: &str) -> Vec<Action> {
        let mut out = Vec::new();
        let Some((mut info, target_id)) = self.pending_registrations.remove(device_id) else {
            return out;
        };

        let is_game = matches!(
            info.device.device_type,
            DeviceType::Flash | DeviceType::Unity | DeviceType::Native
        );
        if is_game {
            let dev_id = info.device.device_id.clone();
            if let Some(existing) = self
                .registry
                .get(&dev_id)
                .and_then(|r| r.info.as_ref().map(|i| i.slot_id))
            {
                if existing > 0 {
                    self.used_slots.remove(&existing);
                }
            }
            info.slot_id = self.allocate_slot();
        } else {
            info.slot_id = 0;
        }
        self.upsert_registry_info(info.clone());

        out.extend(self.make_message_invoke(
            &target_id,
            "onRegister",
            None,
            vec![Value::Bool(true)],
        ));
        out.push(Action::RegistryEvent {
            kind: RegistryEventKind::OnRegister,
            infos: vec![info.clone()],
            success: Some(true),
        });

        if is_game {
            let info_val = Value::Object(Object::BMRegistryInfo(info.clone()));

            out.extend(self.make_message_invoke(
                &target_id,
                "onHostConnected",
                None,
                vec![info_val.clone()],
            ));

            let viewer_ids: Vec<String> = self
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
                    "onHostConnected",
                    None,
                    vec![info_val.clone()],
                ));
            }
        }
        out
    }

    pub fn deny_registration(&mut self, device_id: &str) -> Vec<Action> {
        let mut out = Vec::new();
        let Some((_info, target_id)) = self.pending_registrations.remove(device_id) else {
            return out;
        };
        out.extend(self.make_message_invoke(
            &target_id,
            "onRegister",
            None,
            vec![Value::Bool(false)],
        ));
        out.push(Action::RegistryEvent {
            kind: RegistryEventKind::OnRegister,
            infos: vec![_info],
            success: Some(false),
        });
        out
    }

    pub fn registry(&self) -> &DeviceRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut DeviceRegistry {
        &mut self.registry
    }

    pub fn process_incoming(&mut self, payload: &[u8]) -> Vec<Action> {
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(
            &format!("WASM: Engine::process_incoming len={}", payload.len()).into(),
        );

        let payload_safe = payload.to_vec();

        if payload_safe.is_empty() {
            return Vec::new();
        }

        if payload_safe.len() == 12 {
            if let Some(handshake) =
                crate::codec::externals::handshake::Handshake::from_bytes(&payload_safe)
            {
                #[cfg(target_arch = "wasm32")]
                web_sys::console::log_1(&"WASM: Handshake detected".into());

                return vec![Action::Handshake {
                    current: handshake.current.to_u32(),
                    minimum: handshake.minimum.to_u32(),
                }];
            }
        }

        use crate::codec::externals::bm_packet::BMPacket;
        let mut pkt = Box::new(BMPacket::default());
        match deserialize_packet(&payload_safe, &mut pkt) {
            Ok(_) => {
                #[cfg(target_arch = "wasm32")]
                web_sys::console::log_1(&"WASM: deserialize success, calling handle".into());
                self.handle_deserialized_packet(&pkt)
            }
            Err(e) => {
                #[cfg(target_arch = "wasm32")]
                web_sys::console::log_1(&format!("WASM: deserialize failed: {}", e).into());
                log::warn!("failed to deserialize packet: {}", e);
                Vec::new()
            }
        }
    }

    pub fn process_incoming_udp(&mut self, raw: &[u8]) -> Vec<Action> {
        let mut framed = Vec::with_capacity(4 + raw.len());
        framed.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        framed.extend_from_slice(raw);
        self.process_incoming(&framed)
    }

    fn handle_deserialized_packet(&mut self, pkt: &BMPacket) -> Vec<Action> {
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"WASM: handle_deserialized_packet entry".into());

        let mut out = Vec::new();

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
            out.push(self.push_registry_update(rec));
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
            PacketType::Ping => out.extend(self.handle_ping(pkt, channel, sender_id)),
            PacketType::Ack => out.extend(self.handle_ack(pkt)),
            PacketType::Data => out.extend(self.handle_data(pkt, channel)),
            _ => log::info!(
                "rx packet type {:?} channel {channel} reliability {reliability}",
                pkt_type
            ),
        }
        out
    }

    fn handle_ping(
        &mut self,
        _pkt: &BMPacket,
        channel: i32,
        sender_id: Option<String>,
    ) -> Vec<Action> {
        let mut out = Vec::new();
        log::info!("rx ping");

        if let Some(id) = sender_id {
            out.extend(self.make_packet(
                &id,
                channel,
                Some(Self::default_reliability_for_channel(channel)),
                PacketType::Echo,
                None,
            ));
        }

        out
    }

    fn handle_ack(&mut self, pkt: &BMPacket) -> Vec<Action> {
        let mut out = Vec::new();
        if let Some(rec) = self.device_record_from_packet(pkt) {
            out.push(self.push_registry_update(rec));
        }
        log::info!("rx ack");
        out
    }

    fn handle_data(&mut self, pkt: &BMPacket, channel: i32) -> Vec<Action> {
        let mut out = Vec::new();
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
                            Object::BMInvoke(inv) => out.extend(self.handle_invoke(
                                ReceivedInvoke {
                                    method: inv.method,
                                    return_method: inv.return_method,
                                    params: inv.params,
                                },
                                msg.clone(),
                                Some(pkt.device_id.clone()),
                                channel,
                            )),
                            Object::BMByteChunk(chunk) => {
                                let device_id = pkt.device_id.clone();
                                out.extend(self.handle_chunk(device_id, chunk));
                            }
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
        out
    }

    fn handle_chunk(&mut self, device_id: String, chunk: BMByteChunk) -> Vec<Action> {
        let mut out = Vec::new();
        let set_id = chunk.set_id.clone();

        let buffer = self
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
            return Vec::new();
        }

        let current = end as u32;
        let total = chunk.total_size as u32;

        out.push(Action::ChunkProgress {
            device_id: device_id.clone(),
            set_id: set_id.clone(),
            current,
            total,
        });

        if current >= total {
            if let Some(blob) = self.chunk_buffers.remove(&set_id) {
                out.push(Action::ChunkSetComplete {
                    device_id,
                    set_id,
                    blob,
                });
            }
        }

        out
    }

    fn handle_invoke(
        &mut self,
        inv: ReceivedInvoke,
        raw_bytes: Vec<u8>,
        sender_id: Option<String>,
        channel: i32,
    ) -> Vec<Action> {
        let mut out = Vec::new();
        out.push(Action::Invoke {
            method: inv.method.clone(),
            return_method: inv.return_method.clone(),
            params: inv.params.clone(),
            raw_bytes,
        });

        if let Some(cfg) = self.parse_control_rpc(&inv) {
            out.push(cfg);
        }

        if let Some(handler) = self.rpc_handlers.get(&inv.method).cloned() {
            out.extend(handler(self, &inv.params, sender_id.as_deref(), channel));
        }
        out
    }

    fn rpc_registry_register(
        engine: &mut Engine,
        params: &[Value],
        sender_id: Option<&str>,
        _channel: i32,
    ) -> Vec<Action> {
        let infos = engine.collect_registry_infos(params);
        let success = params.iter().find_map(|p| {
            if let Value::Bool(b) = engine.unwrap_value(p) {
                Some(b)
            } else {
                None
            }
        });
        let mut out = vec![Action::RegistryEvent {
            kind: RegistryEventKind::OnRegister,
            infos: infos.clone(),
            success: success.copied(),
        }];

        if !engine.is_server() {
            return out;
        }

        let Some(target_id) = sender_id else {
            log::warn!("registry.register missing sender id");
            return out;
        };

        let Some(mut info) = infos.first().cloned() else {
            return out;
        };

        if engine.auto_approve_registration {
            let is_game = matches!(
                info.device.device_type,
                DeviceType::Flash | DeviceType::Unity | DeviceType::Native
            );
            if is_game {
                let dev_id = info.device.device_id.clone();
                if let Some(existing) = engine
                    .registry
                    .get(&dev_id)
                    .and_then(|r| r.info.as_ref().map(|i| i.slot_id))
                {
                    if existing > 0 {
                        engine.used_slots.remove(&existing);
                    }
                }
                info.slot_id = engine.allocate_slot();
            } else {
                info.slot_id = 0;
            }
            engine.upsert_registry_info(info.clone());

            out.extend(engine.make_message_invoke(
                target_id,
                "onRegister",
                None,
                vec![Value::Bool(true)],
            ));

            if is_game {
                let info_val = Value::Object(Object::BMRegistryInfo(info.clone()));

                out.extend(engine.make_message_invoke(
                    target_id,
                    "onHostConnected",
                    None,
                    vec![info_val.clone()],
                ));

                let viewer_ids: Vec<String> = engine
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
                    out.extend(engine.make_message_invoke(
                        &vid,
                        "onHostConnected",
                        None,
                        vec![info_val.clone()],
                    ));
                }
            }
        } else {
            engine.pending_registrations.insert(
                info.device.device_id.clone(),
                (info.clone(), target_id.to_string()),
            );
            out.push(Action::RegistryEvent {
                kind: RegistryEventKind::OnRegister,
                infos: vec![info],
                success: None,
            });
        }

        out
    }

    fn rpc_registry_list(
        engine: &mut Engine,
        params: &[Value],
        sender_id: Option<&str>,
        _channel: i32,
    ) -> Vec<Action> {
        let infos = engine.collect_registry_infos(params);
        let mut out = vec![Action::RegistryEvent {
            kind: RegistryEventKind::OnList,
            infos,
            success: None,
        }];

        if !engine.is_server() {
            return out;
        }

        let Some(target_id) = sender_id else {
            return out;
        };
        let viewer_type = engine
            .registry
            .get(target_id)
            .and_then(|r| r.info.as_ref())
            .map(|r| r.device.device_type)
            .unwrap_or(DeviceType::Server);

        let list_infos = engine.registry_infos_for_viewer(viewer_type);
        let mut arr = BMArray::default();
        for r in list_infos {
            arr.push(Value::Object(Object::BMRegistryInfo(r)));
        }
        out.extend(engine.make_message_invoke(
            target_id,
            "onList",
            None,
            vec![Value::Object(Object::BMArray(arr))],
        ));
        out
    }

    fn rpc_registry_relay(
        engine: &mut Engine,
        params: &[Value],
        _sender_id: Option<&str>,
        _channel: i32,
    ) -> Vec<Action> {
        let infos = engine.collect_registry_infos(params);
        let mut out = vec![Action::RegistryEvent {
            kind: RegistryEventKind::DeviceConnectRequested,
            infos: infos.clone(),
            success: None,
        }];

        if !engine.is_server() {
            return out;
        }

        let mut target_id = None;
        let mut inner_invoke = None;

        for p in params {
            match engine.unwrap_value(p) {
                Value::Object(Object::BMRegistryInfo(r)) => {
                    target_id = Some(r.device.device_id.clone());
                }
                Value::Object(Object::BMInvoke(inv)) => {
                    inner_invoke = Some(inv.clone());
                }
                _ => {}
            }
        }

        let Some(target_id) = target_id else {
            return out;
        };
        let Some(inner) = inner_invoke else {
            return out;
        };

        out.extend(engine.make_message_invoke(
            &target_id,
            &inner.method,
            inner.return_method.as_deref(),
            inner.params,
        ));
        out
    }

    fn rpc_on_host_connected(
        engine: &mut Engine,
        params: &[Value],
        _sender_id: Option<&str>,
        _channel: i32,
    ) -> Vec<Action> {
        let infos = engine.collect_registry_infos(params);
        vec![Action::RegistryEvent {
            kind: RegistryEventKind::OnHostConnected,
            infos,
            success: None,
        }]
    }

    fn rpc_registry_update(
        engine: &mut Engine,
        params: &[Value],
        _sender_id: Option<&str>,
        _channel: i32,
    ) -> Vec<Action> {
        let infos = engine.collect_registry_infos(params);
        let mut out = vec![Action::RegistryEvent {
            kind: RegistryEventKind::OnHostUpdate,
            infos: infos.clone(),
            success: None,
        }];

        if !engine.is_server() {
            return out;
        }

        let viewer_ids: Vec<String> = engine
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
            engine.upsert_registry_info(info.clone());
            if !matches!(
                info.device.device_type,
                DeviceType::Flash | DeviceType::Unity | DeviceType::Native
            ) {
                continue;
            }
            let Some(stored) = engine
                .registry
                .get(&info.device.device_id)
                .and_then(|r| r.info.clone())
            else {
                continue;
            };
            for vid in &viewer_ids {
                out.extend(engine.make_message_invoke(
                    vid,
                    "onHostUpdate",
                    None,
                    vec![Value::Object(Object::BMRegistryInfo(stored.clone()))],
                ));
            }
        }
        out
    }

    fn rpc_on_host_disconnected(
        engine: &mut Engine,
        params: &[Value],
        _sender_id: Option<&str>,
        _channel: i32,
    ) -> Vec<Action> {
        let infos = engine.collect_registry_infos(params);
        vec![Action::RegistryEvent {
            kind: RegistryEventKind::OnHostDisconnected,
            infos,
            success: None,
        }]
    }

    fn rpc_device_connect_requested(
        engine: &mut Engine,
        params: &[Value],
        _sender_id: Option<&str>,
        _channel: i32,
    ) -> Vec<Action> {
        let infos = engine.collect_registry_infos(params);
        vec![Action::RegistryEvent {
            kind: RegistryEventKind::DeviceConnectRequested,
            infos,
            success: None,
        }]
    }

    fn allocate_slot(&mut self) -> i16 {
        let mut candidate = 1i16;
        loop {
            if !self.used_slots.contains(&candidate) {
                self.used_slots.insert(candidate);
                return candidate;
            }
            candidate = candidate.wrapping_add(1);
        }
    }

    fn upsert_registry_info(&mut self, mut info: BMRegistryInfo) {
        if let Some(existing) = self
            .registry
            .get(&info.device.device_id)
            .and_then(|r| r.info.clone())
        {
            if info.slot_id <= 0 {
                info.slot_id = existing.slot_id;
            }
            if info.current_players.is_none() {
                info.current_players = existing.current_players;
            }
            if info.max_players.is_none() {
                info.max_players = existing.max_players;
            }
        }
        let record = DeviceRecord::new(info.device.clone(), None, Some(info));
        self.registry.upsert(record);
    }

    fn registry_infos_for_viewer(&self, viewer_type: DeviceType) -> Vec<BMRegistryInfo> {
        let viewer_is_game = matches!(
            viewer_type,
            DeviceType::Flash | DeviceType::Unity | DeviceType::Native
        );
        let mut out = Vec::new();
        for rec in self.registry.snapshot() {
            let Some(info) = rec.info else {
                continue;
            };
            let is_game = matches!(
                info.device.device_type,
                DeviceType::Flash | DeviceType::Unity | DeviceType::Native
            );
            if viewer_is_game {
                if !is_game && info.device.device_type != DeviceType::Server {
                    out.push(info);
                }
            } else if is_game {
                out.push(info);
            }
        }
        out
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

    fn next_invoke_id(&mut self) -> i32 {
        let id = self.invoke_counter;
        self.invoke_counter = if self.invoke_counter == i32::MAX {
            1
        } else {
            self.invoke_counter + 1
        };
        id
    }

    pub fn build_invoke_payload(
        &mut self,
        method: &str,
        return_method: Option<&str>,
        params: Vec<Value>,
    ) -> Result<Vec<u8>> {
        let invoke = BMInvoke {
            id: self.next_invoke_id(),
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
    ) -> Vec<Action> {
        let state = if pressed { "down" } else { "up" };
        self.make_message_invoke(
            target,
            handler,
            None,
            vec![Value::String(state.to_string())],
        )
    }

    pub fn make_dpad_update(&mut self, target: &str, x: i16, y: i16) -> Vec<Action> {
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
    ) -> Vec<Action> {
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
    ) -> Vec<Action> {
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
    ) -> Vec<Action> {
        let params = vec![
            Value::I32(height),
            Value::I32(width),
            Value::String(requester_device_id.to_string()),
        ];
        self.make_message_invoke(target, "RequestXML", None, params)
    }

    pub fn make_on_control_scheme_parsed(&mut self, target: &str, device_id: &str) -> Vec<Action> {
        let params = vec![Value::String(device_id.to_string())];
        self.make_message_invoke(target, "onControlSchemeParsed", None, params)
    }

    pub fn make_simple_invoke_string(
        &mut self,
        target: &str,
        method: &str,
        return_method: Option<&str>,
        param_str: Option<&str>,
    ) -> Vec<Action> {
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
    ) -> Vec<Action> {
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
    ) -> Vec<Action> {
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

    pub fn make_vibrate(&mut self, target: &str) -> Vec<Action> {
        self.make_message_invoke(target, "vibrate", None, vec![])
    }

    pub fn make_update_wallet(&mut self, target: &str) -> Vec<Action> {
        self.make_message_invoke(target, "updateWallet", None, vec![])
    }

    pub fn make_get_cookie(&mut self, target: &str, name: &str) -> Vec<Action> {
        self.make_message_invoke(
            target,
            "getCookie",
            None,
            vec![Value::String(name.to_string())],
        )
    }

    pub fn make_set_cookie(&mut self, target: &str, name: &str, value: &str) -> Vec<Action> {
        self.make_message_invoke(
            target,
            "setCookie",
            None,
            vec![
                Value::String(name.to_string()),
                Value::String(value.to_string()),
            ],
        )
    }

    pub fn make_prompt_trial_upsell(&mut self, target: &str) -> Vec<Action> {
        self.make_message_invoke(target, "promptTrialUpsell", None, vec![])
    }

    pub fn make_wait_for_new_host(&mut self, target: &str, host_device_id: &str) -> Vec<Action> {
        self.make_message_invoke(
            target,
            "WaitForNewHost",
            None,
            vec![Value::String(host_device_id.to_string())],
        )
    }

    pub fn make_set_control_mode(
        &mut self,
        target: &str,
        mode: i32,
        text_content: Option<&str>,
    ) -> Vec<Action> {
        let mut params = vec![Value::I32(mode)];
        if let Some(text) = text_content {
            params.push(Value::String(text.to_string()));
        }
        self.make_message_invoke(target, "SetControlMode", None, params)
    }

    pub fn make_enable_accelerometer(
        &mut self,
        target: &str,
        enabled: bool,
        interval_seconds: Option<f64>,
    ) -> Vec<Action> {
        let mut params = vec![Value::Bool(enabled)];
        if let Some(interval) = interval_seconds {
            params.push(Value::F64(interval));
        }
        self.make_message_invoke(target, "enableAccelerometer", None, params)
    }

    pub fn make_enable_touch(&mut self, target: &str, enabled: bool) -> Vec<Action> {
        self.make_message_invoke(target, "enableTouch", None, vec![Value::Bool(enabled)])
    }

    pub fn make_set_touch_interval(&mut self, target: &str, interval_seconds: f64) -> Vec<Action> {
        self.make_message_invoke(
            target,
            "setTouchInterval",
            None,
            vec![Value::F64(interval_seconds)],
        )
    }

    pub fn make_enable_gyro(&mut self, target: &str, enabled: bool) -> Vec<Action> {
        self.make_message_invoke(target, "enableGyro", None, vec![Value::Bool(enabled)])
    }

    pub fn make_set_gyro_interval(&mut self, target: &str, interval_seconds: f64) -> Vec<Action> {
        self.make_message_invoke(
            target,
            "setGyroInterval",
            None,
            vec![Value::F64(interval_seconds)],
        )
    }

    pub fn make_enable_orientation(&mut self, target: &str, enabled: bool) -> Vec<Action> {
        self.make_message_invoke(
            target,
            "enableOrientation",
            None,
            vec![Value::Bool(enabled)],
        )
    }

    pub fn make_set_orientation_interval(
        &mut self,
        target: &str,
        interval_seconds: f64,
    ) -> Vec<Action> {
        self.make_message_invoke(
            target,
            "setOrientationInterval",
            None,
            vec![Value::F64(interval_seconds)],
        )
    }

    pub fn make_set_reliability_for_touch(
        &mut self,
        target: &str,
        touch_reliability: i32,
        control_reliability: i32,
    ) -> Vec<Action> {
        self.make_message_invoke(
            target,
            "setReliabilityForTouch",
            None,
            vec![
                Value::I32(touch_reliability),
                Value::I32(control_reliability),
            ],
        )
    }

    pub fn make_set_capabilities(&mut self, target: &str, capabilities: u64) -> Vec<Action> {
        self.make_message_invoke(
            target,
            "setCapabilities",
            None,
            vec![Value::U32(capabilities as u32)],
        )
    }

    pub fn make_registry_register(
        &mut self,
        target: &str,
        info: BMRegistryInfo,
        domain: Option<String>,
    ) -> Vec<Action> {
        let mut params = vec![Value::Object(Object::BMRegistryInfo(info))];
        if let Some(d) = domain {
            params.push(Value::String(d));
        }
        let msg = match self.build_invoke_payload("registry.register", Some("onRegister"), params) {
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

    pub fn make_registry_list(&mut self, target: &str) -> Vec<Action> {
        let msg = match self.build_invoke_payload("registry.list", Some("onList"), Vec::new()) {
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
    ) -> Vec<Action> {
        let inner_obj = Value::Object(Object::BMInvoke(inner));
        let params = vec![Value::Object(Object::BMRegistryInfo(dest_info)), inner_obj];
        let msg = match self.build_invoke_payload("registry.relay", Some(""), params) {
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
    ) -> Vec<Action> {
        let inner = BMInvoke {
            id: 0,
            method: "deviceConnectRequested".to_string(),
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
    ) -> Vec<Action> {
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
    ) -> Vec<Action> {
        self.make_message_invoke(target, method, None, params)
    }

    pub fn make_packet(
        &mut self,
        target: &str,
        channel: i32,
        reliability: Option<i32>,
        packet_type: PacketType,
        message: Option<Vec<u8>>,
    ) -> Vec<Action> {
        if target.is_empty() {
            log::warn!("target device id is empty");
            return Vec::new();
        }
        let Some(rec) = self.registry.get(target).cloned() else {
            log::warn!("unknown target device: {target}");
            return Vec::new();
        };

        let rel = reliability.unwrap_or_else(|| Self::default_reliability_for_channel(channel));
        let seq = self.next_sequence(channel);

        #[cfg(target_arch = "wasm32")]
        let timestamp_ms = js_sys::Date::now();

        #[cfg(not(target_arch = "wasm32"))]
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as f64;

        let sender = self.local_device.as_ref().unwrap_or(&rec.core);
        match self.build_packet_bytes(
            sender,
            channel,
            seq,
            timestamp_ms,
            rel,
            packet_type.code(),
            message,
        ) {
            Ok(bytes) => vec![Action::Send {
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

    fn next_sequence(&mut self, channel: i32) -> i32 {
        let entry = self.seq_by_channel.entry(channel).or_insert(0);
        let current = *entry;
        *entry = entry.wrapping_add(1);
        current
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

    pub fn push_registry_update(&mut self, mut record: DeviceRecord) -> Action {
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"WASM: push_registry_update start".into());

        if record.info.is_none() {
            #[cfg(target_arch = "wasm32")]
            web_sys::console::log_1(
                &format!("WASM: checking existing for {}", record.device_id()).into(),
            );

            if let Some(existing) = self.registry.get(record.device_id()) {
                #[cfg(target_arch = "wasm32")]
                web_sys::console::log_1(&"WASM: found existing check".into());
                record.info = existing.info.clone();
            }
        }

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"WASM: upserting...".into());

        self.registry.upsert(record.clone());

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"WASM: upsert done.".into());
        Action::UpdateRegistry { record }
    }

    pub fn drop_device(&mut self, device_id: &str) -> Vec<Action> {
        let mut out = Vec::new();
        if let Some(rec) = self.registry.remove(device_id) {
            if let Some(info) = rec.info {
                if info.slot_id > 0 {
                    self.used_slots.remove(&info.slot_id);
                }

                // If a game disconnected, broadcast onHostDisconnected to all controllers
                // so they can remove it from their host list
                let is_game = matches!(
                    info.device.device_type,
                    DeviceType::Flash | DeviceType::Unity | DeviceType::Native
                );
                if is_game && self.is_server() {
                    let info_val = Value::Object(Object::BMRegistryInfo(info));
                    let viewer_ids: Vec<String> = self
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
                            "onHostDisconnected",
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

    fn parse_control_rpc(&self, inv: &ReceivedInvoke) -> Option<Action> {
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
            "enableAccelerometer" => {
                touch_enabled = None;
                accel_enabled = self.param_bool(&inv.params, 0);
                if let Some(sec) = self.param_f64(&inv.params, 1) {
                    accel_interval_ms = Some((sec * 1000.0) as i32);
                }
            }
            "enableTouch" => {
                touch_enabled = self.param_bool(&inv.params, 0);
            }
            "setTouchInterval" => {
                if let Some(sec) = self.param_f64(&inv.params, 0) {
                    touch_interval_ms = Some((sec * 1000.0) as i32);
                }
            }
            "enableGyro" => {
                gyro_enabled = self.param_bool(&inv.params, 0);
            }
            "setGyroInterval" => {
                if let Some(sec) = self.param_f64(&inv.params, 0) {
                    gyro_interval_ms = Some((sec * 1000.0) as i32);
                }
            }
            "enableOrientation" => {
                orientation_enabled = self.param_bool(&inv.params, 0);
            }
            "setOrientationInterval" => {
                if let Some(sec) = self.param_f64(&inv.params, 0) {
                    orientation_interval_ms = Some((sec * 1000.0) as i32);
                }
            }
            "setReliabilityForTouch" => {
                touch_reliability = self.param_i32(&inv.params, 0);
                control_reliability = self.param_i32(&inv.params, 1);
            }
            "SetControlMode" => {
                control_mode = self.param_i32(&inv.params, 0);
                return_app_id = self.param_string(&inv.params, 1);
            }
            "WaitForNewHost" => {
                portal_id = self.param_string(&inv.params, 0);
                control_mode = Some(3);
            }
            "onPortalId" => {
                return_app_id = self.param_string(&inv.params, 0);
            }
            _ => return None,
        }

        Some(Action::ControlConfig {
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
