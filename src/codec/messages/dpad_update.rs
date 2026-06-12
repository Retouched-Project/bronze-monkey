// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::registry;
use crate::codec::io::{DataInput, DataOutput, Result};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DPadUpdate {
    pub x: i16,
    pub y: i16,
}

impl DPadUpdate {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_DPAD_UPDATE;

    pub fn new(x: i16, y: i16) -> Self {
        Self { x, y }
    }

    pub fn read_from(input: &mut dyn DataInput) -> Result<Self> {
        let x = input.read_short()?;
        let y = input.read_short()?;
        Ok(Self { x, y })
    }

    pub fn write_to(&self, out: &mut dyn DataOutput) -> Result<()> {
        out.write_short(self.x)?;
        out.write_short(self.y)
    }
}
