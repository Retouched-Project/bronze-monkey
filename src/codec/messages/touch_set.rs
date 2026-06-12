// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::registry;
use crate::codec::io::{DataInput, DataOutput, Result};
pub(crate) use crate::codec::messages::touch::Touch;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TouchSet {
    pub touches: HashMap<i32, Touch>,
}

impl TouchSet {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_TOUCH_SET;

    pub fn read_from(input: &mut dyn DataInput) -> Result<Self> {
        let count = input.read_int()?;
        let mut touches = HashMap::with_capacity(count as usize);
        for _ in 0..count {
            let touch = Touch::read_from(input)?;
            touches.insert(touch.id, touch);
        }
        Ok(Self { touches })
    }

    pub fn write_to(&self, out: &mut dyn DataOutput) -> Result<()> {
        out.write_int(self.touches.len() as i32)?;
        let mut sorted_touches: Vec<&Touch> = self.touches.values().collect();
        sorted_touches.sort_by_key(|t| t.id);
        for touch in sorted_touches {
            touch.write_to(out)?;
        }
        Ok(())
    }
}
