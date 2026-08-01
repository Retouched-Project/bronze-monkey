// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::Result;
use crate::codec::bm_stream::BMStream;
use crate::codec::externals::bm_array::BMArray;
use crate::codec::externals::bm_registry_info::BMRegistryInfo;
use crate::codec::messages::acceleration::Acceleration;
use crate::codec::messages::ack_packet::AckPacket;
use crate::codec::messages::bm_byte_chunk::BMByteChunk;
use crate::codec::messages::bm_gyro::BMGyro;
use crate::codec::messages::bm_invoke::BMInvoke;
use crate::codec::messages::bm_parameter::BMParameter;
use crate::codec::messages::dpad_update::DPadUpdate;
use crate::codec::messages::orientation::Orientation;
use crate::codec::messages::ping::Ping;
use crate::codec::messages::shake::Shake;
use crate::codec::messages::string_literal::StringLiteral;
use crate::codec::messages::touch_set::TouchSet;
use crate::devices::bm_address::BMAddress;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Object {
    BMArray(BMArray),
    BMAddress(BMAddress),
    BMRegistryInfo(BMRegistryInfo),
    Acceleration(Acceleration),
    TouchSet(TouchSet),
    AckPacket(AckPacket),
    Ping(Ping),
    StringLiteral(StringLiteral),
    Shake(Shake),
    BMByteChunk(BMByteChunk),
    BMGyro(BMGyro),
    Orientation(Orientation),
    DPadUpdate(DPadUpdate),
    BMInvoke(BMInvoke),
    // Boxed to break the Object -> Value -> Object recursion
    BMParameter(Box<BMParameter>),
}

impl Object {
    pub fn class_id(&self) -> u32 {
        match self {
            Object::BMArray(_) => BMArray::CLASS_ID,
            Object::BMAddress(_) => BMAddress::CLASS_ID,
            Object::BMRegistryInfo(_) => BMRegistryInfo::CLASS_ID,
            Object::Acceleration(_) => Acceleration::CLASS_ID,
            Object::TouchSet(_) => TouchSet::CLASS_ID,
            Object::AckPacket(_) => AckPacket::CLASS_ID,
            Object::Ping(_) => Ping::CLASS_ID,
            Object::StringLiteral(_) => StringLiteral::CLASS_ID,
            Object::Shake(_) => Shake::CLASS_ID,
            Object::BMByteChunk(_) => BMByteChunk::CLASS_ID,
            Object::BMGyro(_) => BMGyro::CLASS_ID,
            Object::Orientation(_) => Orientation::CLASS_ID,
            Object::DPadUpdate(_) => DPadUpdate::CLASS_ID,
            Object::BMInvoke(_) => BMInvoke::CLASS_ID,
            Object::BMParameter(_) => BMParameter::CLASS_ID,
        }
    }

    pub fn decode<B: AsRef<[u8]>>(input: &mut BMStream<B>) -> Result<Self> {
        input.nested(Self::decode_inner)
    }

    fn decode_inner<B: AsRef<[u8]>>(input: &mut BMStream<B>) -> Result<Self> {
        let marker_len = input.read_short()? as usize;
        if marker_len != 1 {
            let bytes = if marker_len > 0 {
                input.read_bytes(marker_len)?
            } else {
                Vec::new()
            };
            let marker = String::from_utf8(bytes).unwrap_or_default();
            return Err(format!("invalid object marker length: {marker_len} ({marker})").into());
        }
        let marker = input.read_bytes(marker_len)?;
        if marker.first().copied() != Some(b'@') {
            let marker_str = String::from_utf8(marker).unwrap_or_default();
            return Err(format!("unknown object marker: {marker_str}").into());
        }

        let id_short = input.read_short()? as u32;

        log::trace!("decode object class_id={id_short}");

        match id_short {
            BMArray::CLASS_ID => Ok(Object::BMArray(BMArray::read_from(input)?)),
            BMAddress::CLASS_ID => Ok(Object::BMAddress(BMAddress::read_from(input)?)),
            BMRegistryInfo::CLASS_ID => {
                Ok(Object::BMRegistryInfo(BMRegistryInfo::read_from(input)?))
            }
            Acceleration::CLASS_ID => Ok(Object::Acceleration(Acceleration::read_from(input)?)),
            TouchSet::CLASS_ID => Ok(Object::TouchSet(TouchSet::read_from(input)?)),
            AckPacket::CLASS_ID => Ok(Object::AckPacket(AckPacket::read_from(input)?)),
            Ping::CLASS_ID => Ok(Object::Ping(Ping::read_from(input)?)),
            StringLiteral::CLASS_ID => Ok(Object::StringLiteral(StringLiteral::read_from(input)?)),
            Shake::CLASS_ID => Ok(Object::Shake(Shake::read_from(input)?)),
            BMByteChunk::CLASS_ID => Ok(Object::BMByteChunk(BMByteChunk::read_from(input)?)),
            BMGyro::CLASS_ID => Ok(Object::BMGyro(BMGyro::read_from(input)?)),
            Orientation::CLASS_ID => Ok(Object::Orientation(Orientation::read_from(input)?)),
            DPadUpdate::CLASS_ID => Ok(Object::DPadUpdate(DPadUpdate::read_from(input)?)),
            BMInvoke::CLASS_ID => Ok(Object::BMInvoke(BMInvoke::read_from(input)?)),
            BMParameter::CLASS_ID => Ok(Object::BMParameter(Box::new(BMParameter::read_from(
                input,
            )?))),
            other => Err(format!("unhandled object class_id={other}").into()),
        }
    }

    pub fn encode(&self, out: &mut BMStream<Vec<u8>>) -> Result<()> {
        out.write_short(self.class_id() as i16)?;
        match self {
            Object::BMArray(x) => x.write_to(out),
            Object::BMAddress(x) => x.write_to(out),
            Object::BMRegistryInfo(x) => x.write_to(out),
            Object::Acceleration(x) => x.write_to(out),
            Object::TouchSet(x) => x.write_to(out),
            Object::AckPacket(x) => x.write_to(out),
            Object::Ping(x) => x.write_to(out),
            Object::StringLiteral(x) => x.write_to(out),
            Object::Shake(x) => x.write_to(out),
            Object::BMByteChunk(x) => x.write_to(out),
            Object::BMGyro(x) => x.write_to(out),
            Object::Orientation(x) => x.write_to(out),
            Object::DPadUpdate(x) => x.write_to(out),
            Object::BMInvoke(x) => x.write_to(out),
            Object::BMParameter(p) => p.write_to(out),
        }
    }

    pub fn encode_with_marker(&self, out: &mut BMStream<Vec<u8>>) -> Result<()> {
        out.write_utf("@")?;
        self.encode(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::bm_stream::MAX_NESTING_DEPTH;
    use crate::codec::messages::bm_encoding::Value;
    use crate::devices::device_core::DeviceCore;
    use crate::types::device_type::DeviceType;

    fn host(n: usize) -> Value {
        let addr = BMAddress {
            address: format!("192.168.1.{}", n % 256),
            unreliable_port: 4000,
            reliable_port: 4001,
        };
        let mut core = DeviceCore::new(format!("dev-{n}"), format!("Game {n}"), DeviceType::Flash);
        core.address = Some(addr.clone());
        Value::Object(Object::BMRegistryInfo(BMRegistryInfo {
            slot_id: 0,
            app_id: "abc123".to_string(),
            current_players: None,
            max_players: None,
            device: core,
            device_address: addr,
        }))
    }

    // Arrays holding one array, `levels` of them, so decoding the outermost
    // reaches exactly `levels` deep.
    fn nest(levels: usize) -> Vec<u8> {
        let mut obj = Object::BMArray(BMArray { items: Vec::new() });
        for _ in 1..levels {
            obj = Object::BMArray(BMArray {
                items: vec![Value::Object(obj)],
            });
        }
        let mut out = BMStream::new();
        obj.encode_with_marker(&mut out).unwrap();
        out.into_inner()
    }

    #[test]
    fn nesting_at_the_limit_decodes() {
        let bytes = nest(MAX_NESTING_DEPTH);
        assert!(Object::decode(&mut BMStream::view(&bytes[..])).is_ok());
    }

    #[test]
    fn nesting_past_the_limit_is_rejected() {
        let bytes = nest(MAX_NESTING_DEPTH + 1);
        let err = Object::decode(&mut BMStream::view(&bytes[..])).unwrap_err();
        assert!(
            err.to_string().contains("nesting depth"),
            "rejected for the wrong reason: {err}"
        );
    }

    #[test]
    fn depth_is_released_between_siblings() {
        // Many shallow objects side by side must not add up to the limit.
        let items: Vec<Value> = (0..MAX_NESTING_DEPTH * 4).map(host).collect();
        let obj = Object::BMArray(BMArray { items });
        let mut out = BMStream::new();
        obj.encode_with_marker(&mut out).unwrap();
        let bytes = out.into_inner();
        assert!(Object::decode(&mut BMStream::view(&bytes[..])).is_ok());
    }

    #[test]
    fn host_list_of_100_round_trips() {
        let items: Vec<Value> = (0..100).map(host).collect();
        let obj = Object::BMArray(BMArray { items });

        let mut out = BMStream::new();
        obj.encode_with_marker(&mut out).unwrap();
        let bytes = out.into_inner();

        let mut input = BMStream::view(&bytes[..]);
        let decoded = Object::decode(&mut input).unwrap();
        match decoded {
            Object::BMArray(a) => assert_eq!(a.items.len(), 100),
            other => panic!("expected BMArray, got {other:?}"),
        }
    }
}
