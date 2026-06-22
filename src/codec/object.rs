// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::bm_array::BMArray;
use crate::codec::externals::bm_registry_info::BMRegistryInfo;
use crate::codec::io::{DataInput, DataOutput, Result};
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
    // Boxed to break the Object -> Value -> Object recursion (a parameter's
    // value may itself be an object); every other variant holds its type directly.
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

    pub fn decode(input: &mut dyn DataInput) -> Result<Self> {
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

        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&format!("WASM: Object::decode class_id={}", id_short).into());

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
            _ => Err(format!("unknown class id: {id_short}").into()),
        }
    }

    pub fn encode(&self, out: &mut dyn DataOutput) -> Result<()> {
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

    pub fn encode_with_marker(&self, out: &mut dyn DataOutput) -> Result<()> {
        out.write_utf("@")?;
        self.encode(out)
    }
}
