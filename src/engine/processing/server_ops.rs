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
                && existing > 0
            {
                self.state.used_slots.remove(&existing);
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

    /// Releases everything held for a peer that is gone, whether its link ended
    /// or it asked to be removed.
    ///
    /// Whoever saw it go is the one who can say so: only that side can tell a
    /// link that ended from one this engine closed itself.
    pub fn peer_gone(&mut self, device_id: &str) -> Vec<Outgoing> {
        let mut out = Vec::new();
        self.server_policy.hidden_hosts.remove(device_id);
        self.server_policy.pending_registrations.remove(device_id);
        self.state.acked_peers.remove(device_id);
        self.input_paths.remove(device_id);

        let Some(rec) = self.state.registry.remove(device_id) else {
            return out;
        };

        // A controller drives one game at a time, so a game leaving ends the
        // session that goes with it. Chunk buffers and input reliability belong
        // to that session rather than to the peer.
        if self.roles.controller() && rec.core.device_type.is_game() {
            self.reset_game_session();
        }

        let Some(info) = rec.info else {
            return out;
        };
        if info.slot_id > 0 {
            self.state.used_slots.remove(&info.slot_id);
        }

        // A registry tells the controllers that could see this host.
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

    /// A game answers a controller's ping with an ack once, and remembers that
    /// it did. A controller that comes back on the same id is owed a fresh one,
    /// or it waits at the first step of the session forever.
    #[test]
    fn a_game_acks_a_controller_that_comes_back() {
        let mut game = Engine::default();
        game.init_local_device(DeviceCore::new(
            "game".to_string(),
            "Game".to_string(),
            DeviceType::Unity,
        ));
        game.configure(EngineConfig {
            endpoint: Some(crate::policy::EndpointMode::Game),
            opens_sessions: false,
            ..Default::default()
        })
        .expect("a game needs nothing else configured");
        joined(&mut game, "phone", DeviceType::Android);

        let ping = ping_from("phone");
        assert_eq!(
            game.process_incoming(&ping, &Default::default())
                .outgoings
                .len(),
            1,
            "the first ping is answered"
        );
        assert!(
            game.process_incoming(&ping, &Default::default())
                .outgoings
                .is_empty(),
            "and is not answered twice"
        );

        game.peer_gone("phone");
        joined(&mut game, "phone", DeviceType::Android);
        assert_eq!(
            game.process_incoming(&ping, &Default::default())
                .outgoings
                .len(),
            1,
            "a controller that came back is owed a fresh ack"
        );
    }

    /// A registry that does not let devices in on its own queues them, and the
    /// operator's answer is what lets them in or turns them away.
    #[test]
    fn a_queued_device_is_let_in_or_turned_away_by_command() {
        for (approve, expected) in [(true, true), (false, false)] {
            let mut eng = registry_server();
            eng.configure(EngineConfig {
                server: true,
                approves_registrations: false,
                ..Default::default()
            })
            .unwrap();
            // A device waiting for approval is known from its packets and
            // nothing more: it is on no registry until someone says so.
            eng.push_registry_update(crate::engine::device_registry::DeviceRecord::new(
                DeviceCore::new(
                    "phone".to_string(),
                    "phone".to_string(),
                    DeviceType::Android,
                ),
                None,
            ));
            queue(&mut eng, "phone");
            assert!(eng.registry_info_of("phone").is_none());

            let device_id = "phone".to_string();
            let out = eng.emit(if approve {
                crate::engine::events::Command::ApproveRegistration { device_id }
            } else {
                crate::engine::events::Command::DenyRegistration { device_id }
            });

            assert_eq!(out.len(), 1, "the device is answered either way");
            assert!(
                eng.server_policy.pending_registrations.is_empty(),
                "and stops waiting"
            );
            assert_eq!(
                eng.registry_info_of("phone").is_some(),
                expected,
                "a device let in is on the registry, one turned away is not"
            );
        }
    }

    fn queue(eng: &mut Engine, id: &str) {
        eng.server_policy.pending_registrations.insert(
            id.to_string(),
            PendingRegistration {
                info: BMRegistryInfo {
                    slot_id: 0,
                    app_id: "app".to_string(),
                    current_players: None,
                    max_players: None,
                    device: DeviceCore::new(id.to_string(), id.to_string(), DeviceType::Android),
                    device_address: BMAddress::new("10.0.0.2".to_string(), 9080, 9081),
                },
                target_id: id.to_string(),
                return_method: Some("onRegister".to_string()),
            },
        );
    }

    /// A device that queued for approval and then left is not still waiting.
    #[test]
    fn a_departing_peer_stops_waiting_for_approval() {
        let mut eng = registry_server();
        eng.configure(EngineConfig {
            server: true,
            approves_registrations: false,
            ..Default::default()
        })
        .unwrap();
        joined(&mut eng, "phone", DeviceType::Android);
        eng.server_policy.pending_registrations.insert(
            "phone".to_string(),
            PendingRegistration {
                info: BMRegistryInfo {
                    slot_id: 0,
                    app_id: "app".to_string(),
                    current_players: None,
                    max_players: None,
                    device: DeviceCore::new(
                        "phone".to_string(),
                        "phone".to_string(),
                        DeviceType::Android,
                    ),
                    device_address: BMAddress::new("10.0.0.2".to_string(), 9080, 9081),
                },
                target_id: "phone".to_string(),
                return_method: None,
            },
        );

        eng.peer_gone("phone");
        assert!(
            eng.server_policy.pending_registrations.is_empty(),
            "a peer that left cannot still be approved"
        );
    }

    /// A ping, as a controller sends one.
    fn ping_from(peer: &str) -> Vec<u8> {
        let mut eng = Engine::default();
        eng.init_local_device(DeviceCore::new(
            peer.to_string(),
            "Phone".to_string(),
            DeviceType::Android,
        ));
        eng.push_registry_update(crate::engine::device_registry::DeviceRecord::new(
            DeviceCore::new("game".to_string(), "Game".to_string(), DeviceType::Unity),
            None,
        ));
        eng.make_ping_packet("game").remove(0).message().to_vec()
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
            .peer_gone("game")
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
