// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::io::{DataInput, DataOutput};
use crate::types::touch_state::TouchState;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct Touch {
    pub id: i32,
    pub x: f32,
    pub y: f32,
    pub screen_width: i16,
    pub screen_height: i16,
    pub state: TouchState,
}

impl Touch {
    pub fn read_from(input: &mut dyn DataInput) -> crate::codec::io::Result<Self> {
        let x = input.read_float()?;
        let y = input.read_float()?;
        let screen_width = input.read_short()?;
        let screen_height = input.read_short()?;
        let state_val = input.read_int()?;
        let state = TouchState::from_value(state_val).ok_or("Invalid TouchState")?;
        let id = input.read_int()?;
        Ok(Self {
            id,
            x,
            y,
            screen_width,
            screen_height,
            state,
        })
    }

    pub fn write_to(&self, out: &mut dyn DataOutput) -> crate::codec::io::Result<()> {
        out.write_float(self.x)?;
        out.write_float(self.y)?;
        out.write_short(self.screen_width)?;
        out.write_short(self.screen_height)?;
        out.write_int(self.state.value())?;
        out.write_int(self.id)
    }
}
