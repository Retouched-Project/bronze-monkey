// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::registry;
use crate::codec::io::{DataInput, DataOutput, Result};
use crate::devices::bm_address::BMAddress;
use crate::devices::device_core::DeviceCore;
use crate::types::device_type::DeviceType;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct BMRegistryInfo {
    pub slot_id: i16,
    pub app_id: String,
    pub current_players: Option<i16>,
    pub max_players: Option<i16>,
    pub device: DeviceCore,
    pub device_address: BMAddress,
}

impl BMRegistryInfo {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_REGISTRY_INFO;

    pub fn read_from(input: &mut dyn DataInput) -> Result<Self> {
        let _ = input.read_short()?;
        let _ = input.read_bytes(1)?;
        let _ = input.read_short()?;
        let device = DeviceCore::read_from(input)?;

        let _ = input.read_short()?;
        let _ = input.read_bytes(1)?;
        let _ = input.read_short()?;
        let device_address = BMAddress::read_from(input)?;

        let app_id = input.read_utf()?;
        let slot_id = input.read_short()?;
        let (current_players, max_players) = if slot_id > 0 {
            (Some(input.read_short()?), Some(input.read_short()?))
        } else {
            (None, None)
        };
        let mut device = device;
        device.address = Some(device_address.clone());
        Ok(Self {
            slot_id,
            app_id,
            current_players,
            max_players,
            device,
            device_address,
        })
    }

    pub fn write_to(&self, out: &mut dyn DataOutput) -> Result<()> {
        let dev_class_id: u32 = match self.device.device_type {
            DeviceType::Flash => registry::BM_CLASS_ID_FLASH_DEVICE,
            DeviceType::Unity => registry::BM_CLASS_ID_UNITY_DEVICE,
            DeviceType::IPhone => registry::BM_CLASS_ID_IPHONE_DEVICE,
            DeviceType::Android => registry::BM_CLASS_ID_ANDROID_DEVICE,
            DeviceType::Native => registry::BM_CLASS_ID_NATIVE_DEVICE,
            DeviceType::Palm => registry::BM_CLASS_ID_PALM_DEVICE,
            DeviceType::Server => registry::BM_CLASS_ID_SERVER_DEVICE,
            _ => registry::BM_CLASS_ID_FLASH_DEVICE,
        };
        out.write_short(1)?;
        out.write_bytes(&[b'@'])?;
        out.write_short(dev_class_id as i16)?;
        self.device.write_to(out)?;

        out.write_short(1)?;
        out.write_bytes(&[b'@'])?;
        out.write_short(registry::BM_CLASS_ID_ADDRESS as i16)?;
        self.device_address.write_to(out)?;
        out.write_utf(&self.app_id)?;
        out.write_short(self.slot_id)?;
        if self.slot_id > 0 {
            out.write_short(self.current_players.unwrap_or(0))?;
            out.write_short(self.max_players.unwrap_or(0))?;
        }
        Ok(())
    }
}
