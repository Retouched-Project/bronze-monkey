// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use std::collections::HashMap;

use crate::devices::device_core::DeviceCore;
use crate::externals::bm_registry_info::BMRegistryInfo;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DeviceRecord {
    pub core: DeviceCore,
    pub class_id: Option<u16>,
    pub info: Option<BMRegistryInfo>,
}

impl DeviceRecord {
    pub fn new(core: DeviceCore, class_id: Option<u16>, info: Option<BMRegistryInfo>) -> Self {
        Self {
            core,
            class_id,
            info,
        }
    }

    pub fn device_id(&self) -> &str {
        &self.core.device_id
    }
}

#[derive(Debug, Default, Clone)]
pub struct DeviceRegistry {
    devices: HashMap<String, DeviceRecord>,
}

impl DeviceRegistry {
    pub fn upsert(&mut self, record: DeviceRecord) -> Option<DeviceRecord> {
        self.devices.insert(record.device_id().to_owned(), record)
    }

    pub fn remove(&mut self, device_id: &str) -> Option<DeviceRecord> {
        self.devices.remove(device_id)
    }

    pub fn get(&self, device_id: &str) -> Option<&DeviceRecord> {
        self.devices.get(device_id)
    }

    pub fn snapshot(&self) -> Vec<DeviceRecord> {
        self.devices.values().cloned().collect()
    }
}
