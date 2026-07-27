// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::bm_stream::BMStream;
use crate::codec::externals::registry;
use crate::codec::Result;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BMGyro {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl BMGyro {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_GYRO;

    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn read_from<B: AsRef<[u8]>>(input: &mut BMStream<B>) -> Result<Self> {
        let x = input.read_float()?;
        let y = input.read_float()?;
        let z = input.read_float()?;
        Ok(Self { x, y, z })
    }

    pub fn write_to(&self, out: &mut BMStream<Vec<u8>>) -> Result<()> {
        out.write_float(self.x)?;
        out.write_float(self.y)?;
        out.write_float(self.z)
    }
}

impl std::fmt::Display for BMGyro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Gyro{{{}, {}, {}}}", self.x, self.y, self.z)
    }
}
