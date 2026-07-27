// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::bm_stream::BMStream;
use crate::codec::externals::registry;
use crate::codec::Result;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringLiteral {
    pub value: String,
}

impl StringLiteral {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_STRING_LITERAL;

    pub fn read_from<B: AsRef<[u8]>>(input: &mut BMStream<B>) -> Result<Self> {
        let value = input.read_utf()?;
        Ok(Self { value })
    }

    pub fn write_to(&self, out: &mut BMStream<Vec<u8>>) -> Result<()> {
        out.write_utf(&self.value)
    }
}
