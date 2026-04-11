// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use std::fmt;
use std::io::Read;

use crate::devices::bm_address::BMAddress;
use crate::devices::device_core::DeviceCore;
use crate::externals::registry;
use crate::io::io::{DataInput, DataOutput, Result};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AckPacket {
    pub device: DeviceCore,
    pub device_address: BMAddress,
}

impl AckPacket {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_ACK_PACKET;

    pub fn new(device: DeviceCore, device_address: BMAddress) -> Self {
        Self {
            device,
            device_address,
        }
    }

    pub fn read_from(input: &mut dyn DataInput) -> Result<Self> {
        let device = DeviceCore::read_from(input)?;

        let addr_id = input.read_unsigned_int()?;
        if addr_id != BMAddress::CLASS_ID {
            return Err(format!(
                "AckPacket: expected BMAddress class id {}, got {}",
                BMAddress::CLASS_ID,
                addr_id
            )
            .into());
        }
        let device_address = BMAddress::read_from(input)?;

        Ok(Self {
            device,
            device_address,
        })
    }

    pub fn write_to(&self, out: &mut dyn DataOutput) -> Result<()> {
        self.device.write_to(out)?;

        out.write_unsigned_int(BMAddress::CLASS_ID)?;
        self.device_address.write_to(out)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(64);
        buf.extend_from_slice(&self.device.device_type.code().to_le_bytes());
        let did = self.device.device_id.as_bytes();
        if did.len() > i16::MAX as usize {
            return Err("deviceId too long".into());
        }
        buf.extend_from_slice(&(did.len() as i16).to_le_bytes());
        buf.extend_from_slice(did);
        let dname = self.device.device_name.as_bytes();
        if dname.len() > i16::MAX as usize {
            return Err("deviceName too long".into());
        }
        buf.extend_from_slice(&(dname.len() as i16).to_le_bytes());
        buf.extend_from_slice(dname);
        buf.extend_from_slice(&(BMAddress::CLASS_ID as u32).to_le_bytes());
        let addr = self.device_address.address.as_bytes();
        if addr.len() > i16::MAX as usize {
            return Err("address too long".into());
        }
        buf.extend_from_slice(&(addr.len() as i16).to_le_bytes());
        buf.extend_from_slice(addr);
        buf.extend_from_slice(&self.device_address.unreliable_port.to_le_bytes());
        buf.extend_from_slice(&self.device_address.reliable_port.to_le_bytes());
        Ok(buf)
    }

    pub fn from_bytes(mut bytes: &[u8]) -> Result<Self> {
        use byteorder::{LittleEndian, ReadBytesExt};
        let device_type_code = bytes.read_i32::<LittleEndian>()?;
        let device_type = crate::types::device_type::DeviceType::for_value(device_type_code)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        let did_len = bytes.read_i16::<LittleEndian>()? as usize;
        let mut did_buf = vec![0u8; did_len];
        bytes.read_exact(&mut did_buf)?;
        let device_id = String::from_utf8(did_buf)?;
        let dname_len = bytes.read_i16::<LittleEndian>()? as usize;
        let mut dname_buf = vec![0u8; dname_len];
        bytes.read_exact(&mut dname_buf)?;
        let device_name = String::from_utf8(dname_buf)?;
        let addr_class = bytes.read_u32::<LittleEndian>()?;
        if addr_class != BMAddress::CLASS_ID {
            return Err("AckPacket addr class id mismatch".into());
        }
        let addr_len = bytes.read_i16::<LittleEndian>()? as usize;
        let mut addr_buf = vec![0u8; addr_len];
        bytes.read_exact(&mut addr_buf)?;
        let address = String::from_utf8(addr_buf)?;
        let unreliable_port = bytes.read_i32::<LittleEndian>()?;
        let reliable_port = bytes.read_i32::<LittleEndian>()?;
        Ok(Self {
            device: DeviceCore::new(device_id, device_name, device_type),
            device_address: BMAddress {
                address,
                unreliable_port,
                reliable_port,
            },
        })
    }
}

impl fmt::Display for AckPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AckPacket [device={}, bmAddress={}]",
            self.device, self.device_address
        )
    }
}
