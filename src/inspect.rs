// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

//! Packet building and inspection
//!
//! A read only counterpart to the engine, for debugging and for programs that
//! want to look at the wire directly. Nothing here reads or writes engine
//! state: no sequence numbers are allocated, no device has to be registered,
//! and the same input always produces the same bytes. That is what keeps it
//! from interfering with normal operation, where packets are built through
//! commands and the consumer never sees the wire at all.

use crate::codec::Result;
use crate::codec::bm_stream::BMStream;
use crate::codec::externals::bm_packet::BMPacket;
use crate::codec::externals::bm_version::BMVersion;
use crate::codec::externals::handshake::Handshake;
use crate::codec::object::Object;
use crate::engine::protocol::{deserialize_packet, serialize_packet};
use crate::types::device_type::DeviceType;
use crate::types::packet_type::PacketType;

use serde::{Deserialize, Serialize};

/// Anything that can arrive on a connection. Not every frame is a BMPacket:
/// the version handshake is length prefixed like one but carries two version
/// fields instead of an object, and it is the first thing either side sends.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[cfg_attr(target_arch = "wasm32", serde(rename_all_fields = "camelCase"))]
pub enum WireView {
    Handshake {
        current: BMVersion,
        minimum: BMVersion,
    },
    // Boxed only because a packet view dwarfs a pair of version fields.
    Packet(Box<PacketView>),
}

/// A packet with its message left in decoded form.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct PacketView {
    #[serde(default)]
    pub sequence: i32,
    #[serde(default)]
    pub channel: i32,
    #[serde(default)]
    pub timestamp: f64,
    #[serde(default)]
    pub rtt: f64,
    #[serde(default)]
    pub packet_type: PacketType,
    #[serde(default)]
    pub device_type: DeviceType,
    #[serde(default)]
    pub device_id: String,
    #[serde(default)]
    pub device_name: String,
    #[serde(default)]
    pub message: Option<Object>,
}

impl PacketView {
    fn into_packet(self) -> Result<BMPacket> {
        let message = match self.message {
            Some(obj) => {
                let mut out = BMStream::new();
                obj.encode_with_marker(&mut out)?;
                Some(out.into_inner())
            }
            None => None,
        };
        Ok(BMPacket {
            sequence: self.sequence,
            channel: self.channel,
            timestamp: self.timestamp,
            rtt: self.rtt,
            packet_type: self.packet_type,
            device_type: self.device_type,
            device_id: self.device_id,
            device_name: self.device_name,
            message,
            address_host: None,
            addr_unreliable_port: 0,
            addr_reliable_port: 0,
        })
    }

    fn from_packet(pkt: BMPacket) -> Result<Self> {
        let message = match &pkt.message {
            Some(msg) if !msg.is_empty() => {
                let mut cur = BMStream::view(msg.as_slice());
                Some(Object::decode(&mut cur)?)
            }
            _ => None,
        };
        Ok(Self {
            sequence: pkt.sequence,
            channel: pkt.channel,
            timestamp: pkt.timestamp,
            rtt: pkt.rtt,
            packet_type: pkt.packet_type,
            device_type: pkt.device_type,
            device_id: pkt.device_id,
            device_name: pkt.device_name,
            message,
        })
    }
}

/// Reads whatever arrived, deciding between a handshake and a packet the same
/// way the engine does. This is the entry point for watching a connection.
pub fn inspect(data: &[u8]) -> Result<WireView> {
    if data.len() == 12
        && let Some(hs) = Handshake::from_bytes(data)
    {
        return Ok(WireView::Handshake {
            current: hs.current,
            minimum: hs.minimum,
        });
    }
    Ok(WireView::Packet(Box::new(inspect_packet(data)?)))
}

/// Serializes anything this module can describe.
pub fn build(view: WireView) -> Result<Vec<u8>> {
    match view {
        WireView::Handshake { current, minimum } => {
            Ok(Handshake::new(current, minimum).to_bytes().to_vec())
        }
        WireView::Packet(packet) => build_packet(*packet),
    }
}

/// Serializes a described packet, length prefix included, ready for a stream.
pub fn build_packet(view: PacketView) -> Result<Vec<u8>> {
    serialize_packet(&view.into_packet()?)
}

/// Serializes a described packet without the length prefix, as a datagram
/// carries it.
pub fn build_datagram(view: PacketView) -> Result<Vec<u8>> {
    let mut bytes = build_packet(view)?;
    bytes.drain(..4);
    Ok(bytes)
}

/// Reads a length prefixed packet back into its described form.
pub fn inspect_packet(data: &[u8]) -> Result<PacketView> {
    let mut pkt = BMPacket::default();
    deserialize_packet(data, &mut pkt)?;
    PacketView::from_packet(pkt)
}

/// Reads a packet that arrived without a length prefix.
pub fn inspect_datagram(data: &[u8]) -> Result<PacketView> {
    let mut framed = Vec::with_capacity(4 + data.len());
    framed.extend_from_slice(&(data.len() as u32).to_le_bytes());
    framed.extend_from_slice(data);
    inspect_packet(&framed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::externals::bm_registry_info::BMRegistryInfo;
    use crate::codec::messages::acceleration::Acceleration;
    use crate::codec::messages::bm_encoding::Value;
    use crate::codec::messages::bm_invoke::BMInvoke;
    use crate::devices::bm_address::BMAddress;
    use crate::devices::device_core::DeviceCore;
    use crate::types::channel_type::ChannelType;

    // BMPacket { message: BMInvoke { onHostUpdate, [BMRegistryInfo] } }
    fn host_update() -> PacketView {
        let addr = BMAddress::new("192.168.1.10".to_string(), 9080, 9081);
        let mut core = DeviceCore::new(
            "game-1".to_string(),
            "Some Game".to_string(),
            DeviceType::Flash,
        );
        core.address = Some(addr.clone());
        let info = BMRegistryInfo {
            slot_id: 2,
            app_id: "abc123".to_string(),
            current_players: Some(1),
            max_players: Some(4),
            device: core,
            device_address: addr,
        };
        PacketView {
            sequence: 7,
            channel: 3,
            timestamp: 1234.5,
            rtt: 0.0,
            packet_type: PacketType::Data,
            device_type: DeviceType::Server,
            device_id: "server".to_string(),
            device_name: "Registry".to_string(),
            message: Some(Object::BMInvoke(BMInvoke {
                id: 0,
                method: "onHostUpdate".to_string(),
                return_method: None,
                params: vec![Value::Object(Object::BMRegistryInfo(info))],
            })),
        }
    }

    #[test]
    fn round_trips_through_the_wire() {
        let view = host_update();
        let bytes = build_packet(view.clone()).unwrap();
        let back = inspect_packet(&bytes).unwrap();

        assert_eq!(back.sequence, view.sequence);
        assert_eq!(back.channel, view.channel);
        assert_eq!(back.timestamp, view.timestamp);
        assert_eq!(back.packet_type, view.packet_type);
        assert_eq!(back.device_type, view.device_type);
        assert_eq!(back.device_id, view.device_id);
        assert_eq!(back.device_name, view.device_name);

        let Some(Object::BMInvoke(inv)) = back.message else {
            panic!("expected a BMInvoke back");
        };
        assert_eq!(inv.method, "onHostUpdate");
        let [Value::Object(Object::BMRegistryInfo(info))] = inv.params.as_slice() else {
            panic!("expected one BMRegistryInfo param");
        };
        assert_eq!(info.slot_id, 2);
        assert_eq!(info.app_id, "abc123");
        assert_eq!(info.device.device_id, "game-1");
        assert_eq!(info.device_address.unreliable_port, 9080);
    }

    #[test]
    fn datagram_form_omits_the_length_prefix() {
        let view = host_update();
        let framed = build_packet(view.clone()).unwrap();
        let bare = build_datagram(view).unwrap();

        assert_eq!(bare.len(), framed.len() - 4);
        assert_eq!(bare, framed[4..]);
        assert_eq!(
            inspect_datagram(&bare).unwrap().device_id,
            inspect_packet(&framed).unwrap().device_id
        );
    }

    #[test]
    fn building_is_pure() {
        // Same view in, same bytes out, however many times it is called.
        let first = build_packet(host_update()).unwrap();
        let second = build_packet(host_update()).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn a_packet_with_no_message_round_trips() {
        let view = PacketView {
            channel: 0,
            packet_type: PacketType::Ping,
            device_id: "me".to_string(),
            ..Default::default()
        };
        let back = inspect_packet(&build_packet(view).unwrap()).unwrap();
        assert!(back.message.is_none());
        assert_eq!(back.packet_type, PacketType::Ping);
    }

    #[test]
    fn msgpack_round_trips_the_view() {
        // The C FFI hands these across as msgpack, with no compiler to catch a
        // shape mismatch, so pin that the enum and its payload survive.
        let view = WireView::Packet(Box::new(host_update()));
        let packed = rmp_serde::to_vec_named(&view).unwrap();
        let back: WireView = rmp_serde::from_slice(&packed).unwrap();
        assert_eq!(build(back).unwrap(), build(view).unwrap());

        let hs = WireView::Handshake {
            current: BMVersion::new(1, 7, 0),
            minimum: BMVersion::new(0, 9, 0),
        };
        let packed = rmp_serde::to_vec_named(&hs).unwrap();
        let back: WireView = rmp_serde::from_slice(&packed).unwrap();
        assert_eq!(build(back).unwrap(), build(hs).unwrap());
    }

    #[test]
    fn a_sensor_packet_round_trips_as_a_datagram() {
        // Sensor objects ride inside a packet like any other message, and go
        // out unreliable, so the datagram form is the one that matters.
        let view = PacketView {
            channel: ChannelType::Acceleration.value(),
            packet_type: PacketType::Data,
            device_type: DeviceType::Android,
            device_id: "phone".to_string(),
            message: Some(Object::Acceleration(Acceleration::new(0.5, -0.25, 1.0))),
            ..Default::default()
        };

        let back = inspect_datagram(&build_datagram(view).unwrap()).unwrap();
        assert_eq!(back.channel, ChannelType::Acceleration.value());
        let Some(Object::Acceleration(a)) = back.message else {
            panic!("expected an Acceleration back");
        };
        assert_eq!((a.x, a.y, a.z), (0.5, -0.25, 1.0));
    }

    #[test]
    fn a_handshake_is_not_mistaken_for_a_packet() {
        let bytes = Handshake::default_version().to_bytes();
        // The old packet only entry point cannot describe it.
        assert!(inspect_packet(&bytes).is_err());

        let WireView::Handshake { current, minimum } = inspect(&bytes).unwrap() else {
            panic!("expected a handshake");
        };
        assert_eq!(current, BMVersion::new(1, 7, 0));
        assert_eq!(minimum, BMVersion::new(0, 9, 0));
    }

    #[test]
    fn handshake_round_trips_through_build() {
        let view = WireView::Handshake {
            current: BMVersion::new(1, 7, 0),
            minimum: BMVersion::new(0, 9, 0),
        };
        let bytes = build(view).unwrap();
        assert_eq!(bytes, Handshake::default_version().to_bytes());
    }

    #[test]
    fn inspect_still_reads_ordinary_packets() {
        let bytes = build(WireView::Packet(Box::new(host_update()))).unwrap();
        let WireView::Packet(view) = inspect(&bytes).unwrap() else {
            panic!("expected a packet");
        };
        assert_eq!(view.device_id, "server");
    }

    #[test]
    fn an_undecodable_message_is_reported_not_swallowed() {
        // A well formed packet whose body claims a class id nothing implements.
        let pkt = BMPacket {
            device_id: "game-1".to_string(),
            message: Some(vec![0x01, 0x00, 0x40, 0xff, 0x00]),
            ..Default::default()
        };
        let bytes = serialize_packet(&pkt).unwrap();

        assert!(inspect_packet(&bytes).is_err());
    }
}
