// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::registry;
use crate::codec::io::{DataInput, DataOutput, Result};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct BMAddress {
    pub address: String,
    pub unreliable_port: i32,
    pub reliable_port: i32,
}

impl BMAddress {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_ADDRESS;

    pub fn new(address: String, unreliable_port: i32, reliable_port: i32) -> Self {
        Self {
            address,
            unreliable_port,
            reliable_port,
        }
    }

    pub fn read_from(input: &mut dyn DataInput) -> Result<Self> {
        let address = input.read_utf()?;
        let unreliable_port = input.read_int()?;
        let reliable_port = input.read_int()?;
        Ok(Self {
            address,
            unreliable_port,
            reliable_port,
        })
    }

    pub fn write_to(&self, out: &mut dyn DataOutput) -> Result<()> {
        out.write_utf(&self.address)?;
        out.write_int(self.unreliable_port)?;
        out.write_int(self.reliable_port)
    }
}

impl std::fmt::Display for BMAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "BMAddress[address={}, unreliablePort={}, reliablePort={}]",
            self.address, self.unreliable_port, self.reliable_port
        )
    }
}
