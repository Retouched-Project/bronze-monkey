// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::devices::bm_address::BMAddress;
use crate::externals::registry;
use crate::io::io::{DataInput, DataOutput, Result};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Ping {
    pub device_id: String,
    pub address: BMAddress,
}

impl Ping {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_PING;

    pub fn read_from(input: &mut dyn DataInput) -> Result<Self> {
        let device_id = input.read_utf()?;

        let addr_id = input.read_unsigned_int()?;
        if addr_id != BMAddress::CLASS_ID {
            return Err(format!(
                "Ping: expected BMAddress class id {}, got {}",
                BMAddress::CLASS_ID,
                addr_id
            )
            .into());
        }
        let address = BMAddress::read_from(input)?;

        Ok(Self { device_id, address })
    }

    pub fn write_to(&self, out: &mut dyn DataOutput) -> Result<()> {
        out.write_utf(&self.device_id)?;
        out.write_unsigned_int(BMAddress::CLASS_ID)?;
        self.address.write_to(out)
    }
}
