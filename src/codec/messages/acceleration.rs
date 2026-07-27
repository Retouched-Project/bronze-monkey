// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::bm_stream::BMStream;
use crate::codec::externals::registry;
use crate::codec::Result;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Acceleration {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Acceleration {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_ACCELERATION;

    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn read_from<B: AsRef<[u8]>>(input: &mut BMStream<B>) -> Result<Self> {
        let x = input.read_double()?;
        let y = input.read_double()?;
        let z = input.read_double()?;
        Ok(Self { x, y, z })
    }

    pub fn write_to(&self, out: &mut BMStream<Vec<u8>>) -> Result<()> {
        out.write_double(self.x)?;
        out.write_double(self.y)?;
        out.write_double(self.z)
    }
}

impl std::fmt::Display for Acceleration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{{}, {}, {}}}", self.x, self.y, self.z)
    }
}
