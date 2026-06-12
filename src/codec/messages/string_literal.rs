// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::registry;
use crate::codec::io::{DataInput, DataOutput, Result};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringLiteral {
    pub value: String,
}

impl StringLiteral {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_STRING_LITERAL;

    pub fn read_from(input: &mut dyn DataInput) -> Result<Self> {
        let value = input.read_utf()?;
        Ok(Self { value })
    }

    pub fn write_to(&self, out: &mut dyn DataOutput) -> Result<()> {
        out.write_utf(&self.value)
    }
}
