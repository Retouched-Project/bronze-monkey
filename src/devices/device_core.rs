// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::io::{DataInput, DataOutput, Result};
use crate::devices::bm_address::BMAddress;
use crate::types::device_type::DeviceType;
use std::fmt::{Debug, Display, Formatter};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct DeviceCore {
    pub device_id: String,
    pub device_name: String,
    pub device_type: DeviceType,
    pub address: Option<BMAddress>,
}

impl DeviceCore {
    pub fn new(id: String, name: String, kind: DeviceType) -> Self {
        Self {
            device_id: id,
            device_name: name,
            device_type: kind,
            address: None,
        }
    }

    pub fn read_from(input: &mut dyn DataInput) -> Result<Self> {
        let type_int = input.read_int()?;
        let device_type = DeviceType::for_value(type_int)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        let device_id = input.read_utf()?;
        let device_name = input.read_utf()?;
        Ok(Self {
            device_id,
            device_name,
            device_type,
            address: None,
        })
    }

    pub fn write_to(&self, out: &mut dyn DataOutput) -> Result<()> {
        out.write_int(self.device_type.code())?;
        out.write_utf(&self.device_id)?;
        out.write_utf(&self.device_name)
    }
}

impl Display for DeviceCore {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Device[id={}, name={}, type={}]",
            self.device_id, self.device_name, self.device_type
        )
    }
}
