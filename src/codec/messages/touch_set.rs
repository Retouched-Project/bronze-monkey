// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::bm_stream::BMStream;
use crate::codec::externals::registry;
use crate::codec::Result;
pub(crate) use crate::codec::messages::touch::Touch;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TouchSet {
    pub touches: Vec<Touch>,
}

impl TouchSet {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_TOUCH_SET;

    pub fn read_from<B: AsRef<[u8]>>(input: &mut BMStream<B>) -> Result<Self> {
        let count = input.read_int()?;
        let mut touches = Vec::with_capacity(count as usize);
        for _ in 0..count {
            touches.push(Touch::read_from(input)?);
        }
        Ok(Self { touches })
    }

    pub fn write_to(&self, out: &mut BMStream<Vec<u8>>) -> Result<()> {
        out.write_int(self.touches.len() as i32)?;
        let mut sorted = self.touches.clone();
        sorted.sort_by_key(|t| t.id);
        for touch in &sorted {
            touch.write_to(out)?;
        }
        Ok(())
    }
}
