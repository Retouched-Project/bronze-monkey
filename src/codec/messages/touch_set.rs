// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::Result;
use crate::codec::bm_stream::BMStream;
use crate::codec::externals::registry;
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

    /// Serializes touches by ascending order.
    /// Avoids heap allocations when there are 16 or fewer touches.
    /// Otherwise falls back to heap allocation.
    pub fn write_to(&self, out: &mut BMStream<Vec<u8>>) -> Result<()> {
        out.write_int(self.touches.len() as i32)?;
        if self.touches.is_empty() {
            return Ok(());
        }
        if self.touches.len() <= 16 {
            let mut stack_refs: [&Touch; 16] = [&self.touches[0]; 16];
            for (i, touch) in self.touches.iter().enumerate() {
                stack_refs[i] = touch;
            }
            let slice = &mut stack_refs[..self.touches.len()];
            slice.sort_unstable_by_key(|t| t.id);
            for touch in slice {
                touch.write_to(out)?;
            }
        } else {
            let mut refs: Vec<&Touch> = self.touches.iter().collect();
            refs.sort_unstable_by_key(|t| t.id);
            for touch in refs {
                touch.write_to(out)?;
            }
        }
        Ok(())
    }
}
