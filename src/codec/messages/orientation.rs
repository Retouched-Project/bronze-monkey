// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::bm_stream::BMStream;
use crate::codec::externals::registry;
use crate::codec::Result;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Orientation {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Orientation {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_ORIENTATION;

    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    pub fn read_from<B: AsRef<[u8]>>(input: &mut BMStream<B>) -> Result<Self> {
        let x = input.read_float()?;
        let y = input.read_float()?;
        let z = input.read_float()?;
        let w = input.read_float()?;
        Ok(Self { x, y, z, w })
    }

    pub fn write_to(&self, out: &mut BMStream<Vec<u8>>) -> Result<()> {
        out.write_float(self.x)?;
        out.write_float(self.y)?;
        out.write_float(self.z)?;
        out.write_float(self.w)
    }
}

impl std::fmt::Display for Orientation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{{}, {}, {}, {}}}", self.x, self.y, self.z, self.w)
    }
}
