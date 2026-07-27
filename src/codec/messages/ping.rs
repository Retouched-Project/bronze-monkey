// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::bm_stream::BMStream;
use crate::codec::externals::registry;
use crate::codec::Result;
use crate::devices::bm_address::BMAddress;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct Ping {
    pub device_id: String,
    pub address: BMAddress,
}

impl Ping {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_PING;

    pub fn read_from<B: AsRef<[u8]>>(input: &mut BMStream<B>) -> Result<Self> {
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

    pub fn write_to(&self, out: &mut BMStream<Vec<u8>>) -> Result<()> {
        out.write_utf(&self.device_id)?;
        out.write_unsigned_int(BMAddress::CLASS_ID)?;
        self.address.write_to(out)
    }
}
