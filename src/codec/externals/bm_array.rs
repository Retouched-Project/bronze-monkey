// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::registry;
use crate::codec::io::{DataInput, DataOutput, Result};
use crate::codec::messages::bm_encoding::{BMEncoding, Value};
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BMArray {
    pub items: Vec<Value>,
}

impl BMArray {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_ARRAY;

    pub fn push(&mut self, v: Value) {
        self.items.push(v);
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn iter(&self) -> impl Iterator<Item = &Value> {
        self.items.iter()
    }

    pub fn read_from(input: &mut dyn DataInput) -> Result<Self> {
        let len = input.read_short()? as i32;
        if len < 0 {
            return Err("negative length in BMArray".into());
        }
        let initial_cap = (len as usize).min(1024);
        let mut items = Vec::with_capacity(initial_cap);
        for _ in 0..len {
            items.push(BMEncoding::decode(input)?);
        }
        Ok(BMArray { items })
    }

    pub fn write_to(&self, out: &mut dyn DataOutput) -> Result<()> {
        if self.items.len() > i16::MAX as usize {
            return Err("BMArray too long".into());
        }
        out.write_short(self.items.len() as i16)?;
        for v in &self.items {
            BMEncoding::encode(v, out)?;
        }
        Ok(())
    }
}

impl fmt::Display for BMArray {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BMArray [length={}]", self.items.len())
    }
}
