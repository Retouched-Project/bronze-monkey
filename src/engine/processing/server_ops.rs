// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use super::Engine;
use crate::codec::messages::bm_encoding::Value;
use crate::codec::object::Object;
use crate::engine::events::Outgoing;
use crate::engine::methods;
use crate::policy::server::PendingRegistration;

impl Engine {
    pub fn approve_registration(&mut self, device_id: &str) -> Vec<Outgoing> {
        let mut out = Vec::new();
        let Some(PendingRegistration {
            mut info,
            target_id,
            return_method,
        }) = self.server_policy.pending_registrations.remove(device_id)
        else {
            return out;
        };

        if info.device.device_type.is_game() {
            let dev_id = info.device.device_id.clone();
            if let Some(existing) = self
                .state
                .registry
                .get(&dev_id)
                .and_then(|r| r.info.as_ref().map(|i| i.slot_id))
            {
                if existing > 0 {
                    self.state.used_slots.remove(&existing);
                }
            }
            info.slot_id = self.state.allocate_slot();
        } else {
            info.slot_id = 0;
        }
        self.state.upsert_registry_info(info.clone());

        if let Some(reply) = Self::reply_method(return_method.as_deref()) {
            out.extend(self.make_message_invoke(&target_id, reply, None, vec![Value::Bool(true)]));
        } else {
            log::warn!(
                "approve_registration for '{target_id}': no return method on record, skipping reply"
            );
        }

        if info.device.device_type.is_game() {
            let info_val = Value::Object(Object::BMRegistryInfo(info.clone()));

            out.extend(self.make_message_invoke(
                &target_id,
                methods::ON_HOST_CONNECTED,
                None,
                vec![info_val.clone()],
            ));

            let viewer_ids: Vec<String> = self
                .state
                .registry
                .snapshot()
                .into_iter()
                .filter_map(|r| r.info)
                .filter(|r| r.device.device_type.is_controller())
                .filter(|r| r.device.device_id != target_id)
                .map(|r| r.device.device_id)
                .collect();
            for vid in viewer_ids {
                out.extend(self.make_message_invoke(
                    &vid,
                    methods::ON_HOST_CONNECTED,
                    None,
                    vec![info_val.clone()],
                ));
            }
        }
        out
    }

    pub fn deny_registration(&mut self, device_id: &str) -> Vec<Outgoing> {
        let mut out = Vec::new();
        let Some(PendingRegistration {
            target_id,
            return_method,
            ..
        }) = self.server_policy.pending_registrations.remove(device_id)
        else {
            return out;
        };
        if let Some(reply) = Self::reply_method(return_method.as_deref()) {
            out.extend(self.make_message_invoke(&target_id, reply, None, vec![Value::Bool(false)]));
        } else {
            log::warn!(
                "deny_registration for '{target_id}': no return method on record, skipping reply"
            );
        }
        out
    }

    pub fn drop_device(&mut self, device_id: &str) -> Vec<Outgoing> {
        let mut out = Vec::new();
        self.server_policy.hidden_hosts.remove(device_id);
        self.state.acked_peers.remove(device_id);
        if let Some(rec) = self.state.registry.remove(device_id) {
            if let Some(info) = rec.info {
                if info.slot_id > 0 {
                    self.state.used_slots.remove(&info.slot_id);
                }

                // If a game disconnected, broadcast onHostDisconnected to all controllers
                // so they can remove it from their host list
                if info.device.device_type.is_game() && self.roles.server {
                    let info_val = Value::Object(Object::BMRegistryInfo(info));
                    let viewer_ids: Vec<String> = self
                        .state
                        .registry
                        .snapshot()
                        .into_iter()
                        .filter_map(|r| r.info)
                        .filter(|r| r.device.device_type.is_controller())
                        .map(|r| r.device.device_id)
                        .collect();
                    for vid in viewer_ids {
                        out.extend(self.make_message_invoke(
                            &vid,
                            methods::ON_HOST_DISCONNECTED,
                            None,
                            vec![info_val.clone()],
                        ));
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::externals::bm_registry_info::BMRegistryInfo;
    use crate::config::EngineConfig;
    use crate::devices::bm_address::BMAddress;
    use crate::devices::device_core::DeviceCore;
    use crate::types::device_type::DeviceType;

    fn registry_server() -> Engine {
        let mut eng = Engine::default();
        eng.init_local_device(DeviceCore::new(
            "reg".to_string(),
            "Registry".to_string(),
            DeviceType::Server,
        ));
        eng.configure(EngineConfig {
            server: true,
            ..Default::default()
        })
        .expect("a server needs nothing else configured");
        eng
    }

    fn joined(eng: &mut Engine, id: &str, kind: DeviceType) {
        let device = DeviceCore::new(id.to_string(), id.to_string(), kind);
        eng.state.upsert_registry_info(BMRegistryInfo {
            slot_id: 0,
            app_id: "app".to_string(),
            current_players: None,
            max_players: None,
            device,
            device_address: BMAddress::new("10.0.0.2".to_string(), 9080, 9081),
        });
    }

    #[test]
    fn a_departing_host_is_announced_to_controllers_only() {
        let mut eng = registry_server();
        joined(&mut eng, "game", DeviceType::Unity);
        joined(&mut eng, "phone", DeviceType::Android);
        joined(&mut eng, "tablet", DeviceType::Palm);
        joined(&mut eng, "other", DeviceType::Flash);
        joined(&mut eng, "unknown", DeviceType::Any);

        let told: Vec<String> = eng
            .drop_device("game")
            .into_iter()
            .map(|o| o.target_device_id)
            .collect();

        // A device that never named its type is not a controller, so it is not
        // owed a host list update, and a second game is not either.
        assert!(told.contains(&"phone".to_string()));
        assert!(told.contains(&"tablet".to_string()));
        assert!(!told.contains(&"unknown".to_string()));
        assert!(!told.contains(&"other".to_string()));
    }
}
