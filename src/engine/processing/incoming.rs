// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use super::{Engine, ReceivedInvoke};
use crate::codec::bm_stream::BMStream;
use crate::codec::externals::bm_packet::BMPacket;
use crate::codec::messages::bm_byte_chunk::BMByteChunk;
use crate::codec::object::Object;
use crate::devices::bm_address::BMAddress;
use crate::devices::device_core::DeviceCore;
use crate::engine::device_registry::DeviceRecord;
use crate::engine::events::{Arrival, Event, ProcessOutput};
use crate::engine::methods;
use crate::engine::protocol::deserialize_message;
use crate::types::packet_type::PacketType;

impl Engine {
    /// Handles one message. A datagram is already one; a stream has to be run
    /// through a [`crate::link::framing::Framer`] first.
    ///
    /// `arrival` is whatever the transport knows about where the bytes came
    /// from. A transport that knows nothing passes `Arrival::default()`.
    pub fn process_incoming(&mut self, message: &[u8], arrival: &Arrival) -> ProcessOutput {
        log::trace!("process_incoming message len={}", message.len());

        let mut out = ProcessOutput::new();
        if message.is_empty() {
            return out;
        }

        let mut pkt = BMPacket::default();
        match deserialize_message(message, &mut pkt) {
            Ok(_) => {
                self.handle_deserialized_packet(&pkt, arrival, &mut out);
            }
            Err(e) => {
                log::warn!("failed to deserialize packet: {}", e);
            }
        }
        out
    }

    fn handle_deserialized_packet(
        &mut self,
        pkt: &BMPacket,
        arrival: &Arrival,
        out: &mut ProcessOutput,
    ) {
        if let Some(event) = self.note_peer(pkt, arrival) {
            out.events.push(event);
        }
        let sender_id = Some(pkt.device_id.clone());

        if arrival.datagram {
            log::trace!("datagram in from '{}'", pkt.device_id);
        }

        let channel = pkt.channel;
        let pkt_type = pkt.packet_type;

        log::trace!("rx packet type={pkt_type:?} channel={channel}");

        match pkt_type {
            PacketType::Ping => self.handle_ping(pkt, sender_id, arrival, out),
            PacketType::Ack => self.handle_ack(pkt, out),
            PacketType::Data => self.handle_data(pkt, channel, out),
            PacketType::Echo => {} // echo is a round-trip of a sent ping, do nothing
            _ => log::debug!("unhandled packet type {pkt_type:?} channel {channel}"),
        }
    }

    fn handle_ping(
        &mut self,
        pkt: &BMPacket,
        sender_id: Option<String>,
        arrival: &Arrival,
        out: &mut ProcessOutput,
    ) {
        let Some(id) = sender_id else {
            return;
        };

        if self.roles.game() {
            if !self.state.acked_peers.contains(&id) {
                let ack = self.make_ack_packet(&id);
                if !ack.is_empty() {
                    self.state.acked_peers.insert(id.clone());
                    out.outgoings.extend(ack);
                }
            }
        } else {
            out.outgoings
                .extend(self.make_echo(&id, pkt, arrival.datagram));
        }
    }

    /// Notes that a peer spoke, and where from.
    ///
    /// A packet header names a device and says nothing about where it is, so a
    /// peer already on record is refreshed in place rather than rebuilt.
    fn note_peer(&mut self, pkt: &BMPacket, arrival: &Arrival) -> Option<Event> {
        let announce = self.roles.server;
        let source = arrival.source.as_deref().filter(|s| !s.is_empty());

        if let Some(rec) = self.state.registry.get_mut(&pkt.device_id) {
            if rec.core.device_name != pkt.device_name {
                rec.core.device_name.clone_from(&pkt.device_name);
            }
            rec.core.device_type = pkt.device_type;
            Self::note_source(&mut rec.core, source);
            return announce.then(|| Event::PeerSeen {
                record: rec.clone(),
            });
        }

        let mut core = DeviceCore::new(
            pkt.device_id.clone(),
            pkt.device_name.clone(),
            pkt.device_type,
        );
        Self::note_source(&mut core, source);
        let record = DeviceRecord::new(core, None);
        let seen = announce.then(|| record.clone());
        self.state.registry.upsert(record);
        seen.map(|record| Event::PeerSeen { record })
    }

    /// The host a peer's bytes came from. The port it listens on for datagrams
    /// is its own to declare, and it does that in its ack.
    fn note_source(core: &mut DeviceCore, source: Option<&str>) {
        let Some(source) = source else {
            return;
        };
        match core.address.as_mut() {
            Some(address) if address.address != source => source.clone_into(&mut address.address),
            Some(_) => {}
            None => core.address = Some(BMAddress::new(source.to_string(), 0, 0)),
        }
    }

    fn note_unreliable_port(&mut self, device_id: &str, port: i32) {
        let Some(rec) = self.state.registry.get_mut(device_id) else {
            log::debug!("'{device_id}' announced port {port} before we had a record for it");
            return;
        };
        let known = match rec.core.address.as_mut() {
            Some(address) => std::mem::replace(&mut address.unreliable_port, port),
            None => {
                rec.core.address = Some(BMAddress::new(String::new(), port, 0));
                0
            }
        };
        if known != port {
            log::info!("'{device_id}' takes datagrams on port {port}");
        }
    }

    fn handle_ack(&mut self, pkt: &BMPacket, out: &mut ProcessOutput) {
        let mut udp_port = 0;
        if let Some(msg) = &pkt.message {
            let mut cur = BMStream::view(msg.as_slice());
            match Object::decode(&mut cur) {
                Ok(Object::AckPacket(ack)) => udp_port = ack.device_address.unreliable_port,
                Ok(_) => {}
                Err(e) => log::debug!("ack decode failed: {e}"),
            }
        }
        if udp_port > 0 {
            self.note_unreliable_port(&pkt.device_id, udp_port);
        }
        let Some(record) = self.state.registry.get(&pkt.device_id).cloned() else {
            return;
        };
        out.events.push(Event::PeerConnected { record, udp_port });

        // The ack is a game saying it is ready to be talked to, and it arrives
        // after the version exchange, so this is the first safe moment to open
        // the session.
        if self.roles.controller() && self.controller_policy.session.automatic {
            let target = pkt.device_id.clone();
            out.outgoings.extend(self.make_session_opening(&target));
        }
    }

    fn handle_data(&mut self, pkt: &BMPacket, channel: i32, out: &mut ProcessOutput) {
        if let Some(msg) = &pkt.message {
            if !msg.is_empty() {
                let mut cur = BMStream::view(msg.as_slice());
                match Object::decode(&mut cur) {
                    Ok(obj) => match obj {
                        Object::BMInvoke(inv) => self.handle_invoke(
                            ReceivedInvoke {
                                method: inv.method,
                                return_method: inv.return_method,
                                params: inv.params,
                            },
                            Some(pkt.device_id.clone()),
                            out,
                        ),
                        Object::BMByteChunk(chunk) => {
                            let device_id = pkt.device_id.clone();
                            self.handle_chunk(device_id, chunk, out);
                        }
                        Object::TouchSet(ts) => out.events.push(Event::Touch {
                            sender: pkt.device_id.clone(),
                            touches: ts.touches,
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
                            log::debug!("rx data object {:?} channel={}", obj.class_id(), channel);
                        }
                    },
                    Err(e) => {
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
                "chunk out of bounds: {}..{} (total {})",
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
        out: &mut ProcessOutput,
    ) {
        log::debug!("rx invoke method={}", inv.method);
        let mut claimed = false;

        if self.roles.controller() {
            if inv.method == methods::SET_RELIABILITY_FOR_TOUCH {
                // Transport config: tracked internally, never surfaced to the consumer.
                let touch = self.param_i32(&inv.params, 0);
                let sensors = self.param_i32(&inv.params, 1);
                self.set_input_reliability(touch, sensors);
                claimed = true;
            } else if let Some(cfg) = self.parse_control_rpc(&inv) {
                out.events.push(Event::ControlConfig(cfg));
                claimed = true;
            }
        }

        if let Some(handler) = self.resolve_handler(&inv.method) {
            let mut ctx = super::RpcContext {
                engine: self,
                inv: &inv,
                sender_id: sender_id.as_deref(),
                out,
            };
            handler(&mut ctx);
            claimed = true;
        }

        if !claimed && self.roles.game() && self.game_policy.button_handlers.contains(&inv.method) {
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

    pub fn push_registry_update(&mut self, mut record: DeviceRecord) -> Option<Event> {
        if let Some(existing) = self.state.registry.get(record.device_id()) {
            if record.info.is_none() {
                record.info = existing.info.clone();
            }
            // A packet header names a device but says nothing about where it
            // is, so a record built from one must not lose an address.
            if let Some(known) = existing.core.address.clone() {
                record
                    .core
                    .address
                    .get_or_insert_with(BMAddress::default)
                    .fill_gaps_from(&known);
            }
        }

        let seen = self.roles.server.then(|| record.clone());
        self.state.registry.upsert(record);
        seen.map(|record| Event::PeerSeen { record })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::externals::bm_registry_info::BMRegistryInfo;
    use crate::config::EngineConfig;
    use crate::engine::events::Via;
    use crate::policy::EndpointMode;
    use crate::types::device_type::DeviceType;

    /// A game that will answer with an ack naming the port it listens on.
    fn game_acking(from: &str, unreliable_port: i32) -> Vec<u8> {
        let mut eng = Engine::default();
        let mut core = DeviceCore::new(from.to_string(), "Game".to_string(), DeviceType::Unity);
        core.address = Some(BMAddress::new("0.0.0.0".to_string(), unreliable_port, 0));
        eng.init_local_device(core);
        eng.configure(EngineConfig {
            endpoint: Some(EndpointMode::Game),
            opens_sessions: false,
            ..Default::default()
        })
        .unwrap();
        eng.push_registry_update(DeviceRecord::new(
            DeviceCore::new("me".to_string(), "Me".to_string(), DeviceType::Android),
            None,
        ));
        eng.make_ack_packet("me").remove(0).message().to_vec()
    }

    fn controller_knowing(game: &str) -> Engine {
        let mut eng = Engine::default();
        eng.init_local_device(DeviceCore::new(
            "me".to_string(),
            "Me".to_string(),
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

    fn address_of<'a>(eng: &'a Engine, id: &str) -> Option<&'a BMAddress> {
        eng.state.registry.get(id)?.core.address.as_ref()
    }

    /// Any ordinary message from a peer. It names the sender in its header and
    /// carries no address at all.
    fn plain_packet_from(peer: &str) -> Vec<u8> {
        let mut eng = Engine::default();
        eng.init_local_device(DeviceCore::new(
            peer.to_string(),
            "Game".to_string(),
            DeviceType::Unity,
        ));
        eng.push_registry_update(DeviceRecord::new(
            DeviceCore::new("me".to_string(), "Me".to_string(), DeviceType::Android),
            None,
        ));
        eng.make_message_invoke("me", "onHostUpdate", None, Vec::new())
            .remove(0)
            .message()
            .to_vec()
    }

    /// A ping carries a peer's address so the other side can reach it, and one
    /// that came unreliably is asking whether that path works in reverse.
    fn ping_from(peer: &str) -> Vec<u8> {
        let mut eng = Engine::default();
        eng.init_local_device(DeviceCore::new(
            peer.to_string(),
            "Game".to_string(),
            DeviceType::Unity,
        ));
        eng.push_registry_update(DeviceRecord::new(
            DeviceCore::new("me".to_string(), "Me".to_string(), DeviceType::Android),
            None,
        ));
        eng.make_ping_packet("me").remove(0).message().to_vec()
    }

    #[test]
    fn an_echo_goes_back_the_way_the_ping_came() {
        let mut eng = controller_knowing("game");
        eng.configure(EngineConfig {
            endpoint: Some(EndpointMode::Controller),
            opens_sessions: false,
            datagrams: true,
            ..Default::default()
        })
        .unwrap();
        eng.process_incoming(&game_acking("game", 9049), &Arrival::default());

        let ping = ping_from("game");
        let mut sent = BMPacket::default();
        deserialize_message(&ping, &mut sent).expect("a ping is a message");

        let over_stream = eng.process_incoming(&ping, &Arrival::default());
        assert_eq!(over_stream.outgoings.len(), 1);
        assert_eq!(over_stream.outgoings[0].via, Via::Stream);

        // An echo returns what the ping carried.
        let mut echoed = BMPacket::default();
        deserialize_message(over_stream.outgoings[0].message(), &mut echoed)
            .expect("and so is an echo");
        assert_eq!(echoed.packet_type, PacketType::Echo);
        assert_eq!(echoed.sequence, sent.sequence);
        assert_eq!(echoed.timestamp, sent.timestamp);
        assert_eq!(echoed.channel, sent.channel);
        assert_eq!(echoed.message, sent.message);

        let over_datagram = eng.process_incoming(
            &ping,
            &Arrival {
                datagram: true,
                ..Default::default()
            },
        );
        assert_eq!(over_datagram.outgoings.len(), 1);
        assert!(
            over_datagram.outgoings[0].via.is_datagram(),
            "answering a datagram over the stream tells the sender nothing"
        );
    }

    #[test]
    fn a_peer_is_placed_where_its_bytes_came_from() {
        let mut eng = controller_knowing("game");
        eng.process_incoming(
            &game_acking("game", 9080),
            &Arrival {
                source: Some("192.168.1.5".to_string()),
                ..Default::default()
            },
        );

        let address = address_of(&eng, "game").expect("the ack should have placed it");
        // The host is the transport's to report, the port is the peer's to name.
        assert_eq!(address.address, "192.168.1.5");
        assert_eq!(address.unreliable_port, 9080);
    }

    #[test]
    fn a_port_learned_from_an_ack_outlives_the_next_packet() {
        let mut eng = controller_knowing("game");
        eng.process_incoming(&game_acking("game", 9049), &Arrival::default());
        assert_eq!(
            address_of(&eng, "game").map(|a| a.unreliable_port),
            Some(9049)
        );

        // Anything at all from the same peer, carrying no address of its own.
        eng.process_incoming(&plain_packet_from("game"), &Arrival::default());
        assert_eq!(
            address_of(&eng, "game").map(|a| a.unreliable_port),
            Some(9049),
            "a later packet must not erase where the peer can be reached"
        );
    }

    /// A host list says what a host claims about itself. None of it reaches the
    /// record of how to reach that host, which holds only what was observed.
    #[test]
    fn a_host_list_entry_never_reaches_the_observed_address() {
        let mut eng = controller_knowing("game");
        eng.process_incoming(&game_acking("game", 9049), &Arrival::default());

        let mut listed = DeviceCore::new("game".to_string(), "Game".to_string(), DeviceType::Unity);
        listed.address = Some(BMAddress::new("10.0.0.9".to_string(), 0, 8088));
        eng.state.upsert_registry_info(BMRegistryInfo {
            slot_id: 1,
            app_id: "app".to_string(),
            current_players: None,
            max_players: None,
            device: listed,
            device_address: BMAddress::new("10.0.0.9".to_string(), 0, 8088),
        });

        let address = address_of(&eng, "game").expect("the host is still known");
        assert_eq!(
            address.unreliable_port, 9049,
            "the ack named this, a list only claims"
        );
        assert_eq!(address.reliable_port, 0, "nothing observed a reliable port");
        assert_ne!(
            address.address, "10.0.0.9",
            "a claimed host is not a host we found the peer at"
        );

        // What the host claimed is still there for a caller that wants it.
        let claimed = eng
            .registry_info_of("game")
            .expect("the claim is kept whole");
        assert_eq!(claimed.device_address.address, "10.0.0.9");
        assert_eq!(claimed.device_address.reliable_port, 8088);
    }

    #[test]
    fn a_claimed_port_must_not_outrank_an_observed_one() {
        let mut eng = controller_knowing("game");
        eng.process_incoming(&game_acking("game", 9049), &Arrival::default());

        // A host list update arriving later, claiming a different port.
        let mut listed = DeviceCore::new("game".to_string(), "Game".to_string(), DeviceType::Unity);
        listed.address = Some(BMAddress::new("10.0.0.9".to_string(), 1234, 8088));
        eng.state.upsert_registry_info(BMRegistryInfo {
            slot_id: 1,
            app_id: "app".to_string(),
            current_players: None,
            max_players: None,
            device: listed,
            device_address: BMAddress::new("10.0.0.9".to_string(), 1234, 8088),
        });

        assert_eq!(
            address_of(&eng, "game").map(|a| a.unreliable_port),
            Some(9049),
            "the ack observed this port, the list only claims one"
        );
    }

    #[test]
    fn a_relayed_arrival_still_carries_the_port() {
        let mut eng = controller_knowing("game");
        eng.process_incoming(&game_acking("game", 9080), &Arrival::default());

        let address = address_of(&eng, "game").expect("the ack alone should record a port");
        assert_eq!(address.unreliable_port, 9080);
        assert!(
            address.address.is_empty(),
            "a transport that reports no source invents none"
        );
    }
}
