// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::bm_registry_info::BMRegistryInfo;
use crate::devices::device_core::DeviceCore;
use crate::engine::device_registry::{DeviceRecord, DeviceRegistry};
use crate::types::device_type::DeviceType;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default, Clone)]
pub struct EngineState {
    pub(crate) registry: DeviceRegistry,
    pub(crate) seq_by_channel: HashMap<i32, i32>,
    pub(crate) local_device: Option<DeviceCore>,
    pub(crate) chunk_buffers: HashMap<String, Vec<u8>>,
    pub(crate) invoke_counter: i32,
    pub(crate) used_slots: HashSet<i16>,
    pub(crate) acked_peers: HashSet<String>,
}

impl EngineState {
    pub fn new() -> Self {
        Self {
            registry: DeviceRegistry::default(),
            seq_by_channel: HashMap::new(),
            local_device: None,
            chunk_buffers: HashMap::new(),
            invoke_counter: 1,
            used_slots: HashSet::new(),
            acked_peers: HashSet::new(),
        }
    }

    pub fn registry(&self) -> &DeviceRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut DeviceRegistry {
        &mut self.registry
    }

    pub fn local_device(&self) -> Option<&DeviceCore> {
        self.local_device.as_ref()
    }

    pub fn init_local_device(&mut self, core: DeviceCore) {
        self.local_device = Some(core);
    }

    pub(crate) fn next_sequence(&mut self, channel: i32) -> i32 {
        let entry = self.seq_by_channel.entry(channel).or_insert(0);
        let current = *entry;
        *entry = entry.wrapping_add(1);
        current
    }

    pub(crate) fn next_invoke_id(&mut self) -> i32 {
        let id = self.invoke_counter;
        self.invoke_counter = if self.invoke_counter == i32::MAX {
            1
        } else {
            self.invoke_counter + 1
        };
        id
    }

    pub(crate) fn allocate_slot(&mut self) -> i16 {
        let mut candidate = 1i16;
        loop {
            if !self.used_slots.contains(&candidate) {
                self.used_slots.insert(candidate);
                return candidate;
            }
            candidate = candidate.wrapping_add(1);
        }
    }

    pub(crate) fn upsert_registry_info(&mut self, mut info: BMRegistryInfo) {
        if let Some(existing) = self
            .registry
            .get(&info.device.device_id)
            .and_then(|r| r.info.clone())
        {
            if info.slot_id <= 0 {
                info.slot_id = existing.slot_id;
            }
            if info.current_players.is_none() {
                info.current_players = existing.current_players;
            }
            if info.max_players.is_none() {
                info.max_players = existing.max_players;
            }
        }
        let record = DeviceRecord::new(info.device.clone(), None, Some(info));
        self.registry.upsert(record);
    }

    pub(crate) fn registry_infos_for_viewer(
        &self,
        viewer_type: DeviceType,
        hidden_hosts: &HashSet<String>,
    ) -> Vec<BMRegistryInfo> {
        let viewer_is_game = matches!(
            viewer_type,
            DeviceType::Flash | DeviceType::Unity | DeviceType::Native
        );
        let mut out = Vec::new();
        for rec in self.registry.snapshot() {
            let Some(info) = rec.info else {
                continue;
            };
            let is_game = matches!(
                info.device.device_type,
                DeviceType::Flash | DeviceType::Unity | DeviceType::Native
            );
            if viewer_is_game {
                if !is_game && info.device.device_type != DeviceType::Server {
                    out.push(info);
                }
            } else if is_game && !hidden_hosts.contains(&info.device.device_id) {
                out.push(info);
            }
        }
        out
    }
}
