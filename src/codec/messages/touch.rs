// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::bm_stream::BMStream;
use crate::codec::Result;
use crate::types::touch_state::TouchState;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct Touch {
    pub id: i32,
    pub x: f64,
    pub y: f64,
    pub screen_width: i16,
    pub screen_height: i16,
    pub state: TouchState,
}

impl Touch {
    pub fn read_from<B: AsRef<[u8]>>(input: &mut BMStream<B>) -> Result<Self> {
        let x = input.read_float()? as f64;
        let y = input.read_float()? as f64;
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

    pub fn write_to(&self, out: &mut BMStream<Vec<u8>>) -> Result<()> {
        out.write_float(self.x as f32)?;
        out.write_float(self.y as f32)?;
        out.write_short(self.screen_width)?;
        out.write_short(self.screen_height)?;
        out.write_int(self.state.value())?;
        out.write_int(self.id)
    }
}
