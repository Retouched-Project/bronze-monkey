// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use super::{Engine, ReceivedInvoke};
use crate::codec::bm_stream::BMStream;
use crate::codec::externals::bm_packet::BMPacket;
use crate::codec::messages::bm_byte_chunk::BMByteChunk;
use crate::codec::object::Object;
use crate::devices::device_core::DeviceCore;
use crate::engine::device_registry::DeviceRecord;
use crate::engine::events::{Event, ProcessOutput};
use crate::engine::methods;
use crate::engine::protocol::deserialize_message;
use crate::types::packet_type::PacketType;

impl Engine {
    /// Handles one message. A datagram is already one; a stream has to be run
    /// through a [`crate::link::framing::Framer`] first.
    pub fn process_incoming(&mut self, message: &[u8]) -> ProcessOutput {
        log::trace!("process_incoming message len={}", message.len());

        let mut out = ProcessOutput::new();
        if message.is_empty() {
            return out;
        }

        let mut pkt = BMPacket::default();
        match deserialize_message(message, &mut pkt) {
            Ok(_) => {
                self.handle_deserialized_packet(&pkt, &mut out);
            }
            Err(e) => {
                log::warn!("failed to deserialize packet: {}", e);
            }
        }
        out
    }

    fn handle_deserialized_packet(&mut self, pkt: &BMPacket, out: &mut ProcessOutput) {
        let sender_id = if let Some(rec) = self.device_record_from_packet(pkt) {
            let id = rec.core.device_id.clone();
            if let Some(event) = self.push_registry_update(rec) {
                out.events.push(event);
            }
            Some(id)
        } else {
            None
        };

        let channel = pkt.channel;
        let pkt_type = pkt.packet_type;

        log::trace!("rx packet type={pkt_type:?} channel={channel}");

        match pkt_type {
            PacketType::Ping => self.handle_ping(pkt, channel, sender_id, out),
            PacketType::Ack => self.handle_ack(pkt, out),
            PacketType::Data => self.handle_data(pkt, channel, out),
            PacketType::Echo => {} // echo is a round-trip of a sent ping, do nothing
            _ => log::debug!("unhandled packet type {pkt_type:?} channel {channel}"),
        }
    }

    fn handle_ping(
        &mut self,
        pkt: &BMPacket,
        channel: i32,
        sender_id: Option<String>,
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
            out.outgoings.extend(self.make_packet(
                &id,
                channel,
                Some(Self::default_reliability_for_channel(channel)),
                PacketType::Echo,
                pkt.message.clone(),
            ));
        }
    }

    fn handle_ack(&mut self, pkt: &BMPacket, out: &mut ProcessOutput) {
        let Some(rec) = self.device_record_from_packet(pkt) else {
            return;
        };
        let mut udp_port = 0;
        if let Some(msg) = &pkt.message {
            let mut cur = BMStream::view(msg.as_slice());
            match Object::decode(&mut cur) {
                Ok(Object::AckPacket(ack)) => udp_port = ack.device_address.unreliable_port,
                Ok(_) => {}
                Err(e) => log::debug!("ack decode failed: {e}"),
            }
        }
        out.events.push(Event::PeerConnected {
            record: rec,
            udp_port,
        });
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

    fn device_record_from_packet(&self, pkt: &BMPacket) -> Option<DeviceRecord> {
        let core = DeviceCore::new(
            pkt.device_id.clone(),
            pkt.device_name.clone(),
            pkt.device_type,
        );
        Some(DeviceRecord::new(core, None, None))
    }

    pub fn push_registry_update(&mut self, mut record: DeviceRecord) -> Option<Event> {
        if record.info.is_none() {
            if let Some(existing) = self.state.registry.get(record.device_id()) {
                record.info = existing.info.clone();
            }
        }

        self.state.registry.upsert(record.clone());
        self.roles.server.then(|| Event::PeerSeen { record })
    }
}
