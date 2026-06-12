// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::registry;
use crate::codec::io::{DataInput, DataOutput, Result};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BMByteChunk {
    pub set_id: String,
    pub start_byte: i32,
    pub chunk_size: i32,
    pub total_size: i32,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

impl BMByteChunk {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_BYTE_CHUNK;

    pub fn read_from(input: &mut dyn DataInput) -> Result<Self> {
        let set_id = input.read_utf()?;
        let start_byte = input.read_int()?;
        let chunk_size = input.read_int()?;
        let total_size = input.read_int()?;
        let data = input.read_bytes(chunk_size as usize)?;
        Ok(Self {
            set_id,
            start_byte,
            chunk_size,
            total_size,
            data,
        })
    }

    pub fn write_to(&self, out: &mut dyn DataOutput) -> Result<()> {
        out.write_utf(&self.set_id)?;
        out.write_int(self.start_byte)?;
        out.write_int(self.chunk_size)?;
        out.write_int(self.total_size)?;
        out.write_bytes(&self.data)
    }
}
