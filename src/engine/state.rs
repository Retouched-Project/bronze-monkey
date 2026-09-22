// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::bm_registry_info::BMRegistryInfo;
use crate::devices::device_core::DeviceCore;
use crate::engine::device_registry::{DeviceRecord, DeviceRegistry};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct EngineState {
    pub(crate) registry: DeviceRegistry,
    pub(crate) seq_by_channel: HashMap<i32, i32>,
    pub(crate) local_device: Option<DeviceCore>,
    pub(crate) local_info: Option<BMRegistryInfo>,
    pub(crate) chunk_buffers: HashMap<String, Vec<u8>>,
    pub(crate) invoke_counter: i32,
    pub(crate) used_slots: HashSet<i16>,
    pub(crate) acked_peers: HashSet<String>,
    pub(crate) peers_answered: HashSet<String>,
}

impl Default for EngineState {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineState {
    pub fn new() -> Self {
        Self {
            registry: DeviceRegistry::default(),
            seq_by_channel: HashMap::new(),
            local_device: None,
            local_info: None,
            chunk_buffers: HashMap::new(),
            invoke_counter: 1,
            used_slots: HashSet::new(),
            acked_peers: HashSet::new(),
            peers_answered: HashSet::new(),
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

    pub(crate) fn registry_info_of(&self, device_id: &str) -> Option<BMRegistryInfo> {
        self.registry.get(device_id)?.info.clone()
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

    pub(crate) fn allocate_slot(&mut self) -> Option<i16> {
        let free = (1..=i16::MAX).find(|slot| !self.used_slots.contains(slot))?;
        self.used_slots.insert(free);
        Some(free)
    }

    pub(crate) fn upsert_registry_info(&mut self, mut info: BMRegistryInfo) {
        let known_address = self
            .registry
            .get(&info.device.device_id)
            .and_then(|r| r.core.address.clone());

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

        // A registry lists what a host said about itself, which a host is free
        // to get wrong, so none of it reaches the record of how to talk to that
        // host. The claim stays on the info, intact and ready to go back out on
        // the wire, for a caller that wants to read it.
        let mut core = info.device.clone();
        core.address = known_address;

        let record = DeviceRecord::new(core, Some(info));
        self.registry.upsert(record);
    }

    /// The hosts a registry list answers with, whoever asked. A list carries
    /// hosts and nothing else.
    ///
    /// A host that has hidden itself is left out for everyone, including
    /// itself.
    pub(crate) fn visible_host_infos(&self, hidden_hosts: &HashSet<String>) -> Vec<BMRegistryInfo> {
        let mut out = Vec::new();
        for rec in self.registry.snapshot() {
            let Some(info) = rec.info else {
                continue;
            };
            if info.device.device_type.is_game() && !hidden_hosts.contains(&info.device.device_id) {
                out.push(info);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::bm_address::BMAddress;
    use crate::types::device_type::DeviceType;

    #[test]
    fn slots_keep_being_handed_out_past_the_fifteen_that_have_colours() {
        let mut state = EngineState::new();
        let handed: Vec<i16> = (0..40).map(|_| state.allocate_slot().unwrap()).collect();

        assert_eq!(handed, (1..=40).collect::<Vec<i16>>());
    }

    #[test]
    fn an_allocated_slot_is_never_zero_or_negative_however_many_are_taken() {
        let mut state = EngineState::new();
        state.used_slots = (1..i16::MAX).collect();

        assert_eq!(state.allocate_slot(), Some(i16::MAX));
        assert_eq!(
            state.allocate_slot(),
            None,
            "a full registry must refuse rather than wrap"
        );
    }

    #[test]
    fn a_released_slot_is_handed_out_again() {
        let mut state = EngineState::new();
        for _ in 0..5 {
            state.allocate_slot().unwrap();
        }
        state.used_slots.remove(&3);

        assert_eq!(state.allocate_slot(), Some(3));
    }

    fn registered(id: &str, kind: DeviceType) -> DeviceRecord {
        let device = DeviceCore::new(id.to_string(), id.to_string(), kind);
        let info = BMRegistryInfo {
            slot_id: 0,
            app_id: "app".to_string(),
            current_players: None,
            max_players: None,
            device: device.clone(),
            device_address: BMAddress::new("10.0.0.2".to_string(), 9080, 9081),
        };
        DeviceRecord::new(device, Some(info))
    }

    fn ids(infos: &[BMRegistryInfo]) -> Vec<&str> {
        infos.iter().map(|i| i.device.device_id.as_str()).collect()
    }

    #[test]
    fn a_list_carries_hosts_and_leaves_controllers_out() {
        let mut state = EngineState::new();
        state
            .registry
            .upsert(registered("flash", DeviceType::Flash));
        state
            .registry
            .upsert(registered("unity", DeviceType::Unity));
        state
            .registry
            .upsert(registered("native", DeviceType::Native));
        state
            .registry
            .upsert(registered("phone", DeviceType::Android));
        state.registry.upsert(registered("reg", DeviceType::Server));

        let infos = state.visible_host_infos(&HashSet::new());
        let mut listed = ids(&infos);
        listed.sort();
        assert_eq!(listed, ["flash", "native", "unity"]);
    }

    #[test]
    fn a_hidden_host_is_left_out() {
        let mut state = EngineState::new();
        state
            .registry
            .upsert(registered("shown", DeviceType::Unity));
        state
            .registry
            .upsert(registered("hidden", DeviceType::Unity));
        let hidden = HashSet::from(["hidden".to_string()]);

        assert_eq!(ids(&state.visible_host_infos(&hidden)), ["shown"]);
    }
}
