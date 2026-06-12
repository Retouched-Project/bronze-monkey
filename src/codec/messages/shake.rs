// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::registry;
use crate::codec::io::{DataInput, DataOutput, Result};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Shake;

impl Shake {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_SHAKE;

    pub fn read_from(_input: &mut dyn DataInput) -> Result<Self> {
        Ok(Self)
    }

    pub fn write_to(&self, out: &mut dyn DataOutput) -> Result<()> {
        out.write_int(0)
    }
}
