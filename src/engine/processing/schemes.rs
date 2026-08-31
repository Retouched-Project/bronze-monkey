// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

//! The schemes a game holds, and which device gets which.
//!
//! A host serves several control schemes, one per kind of device, and picks
//! between them when a controller asks. Keeping them here is what lets the
//! engine answer `RequestXML` on the game's behalf, so a game never marshals a
//! scheme at request time.

use crate::controls::parser::BMApplicationSchemeParser;
use crate::controls::{ControlScheme, writer};
use std::collections::HashMap;

pub(crate) const DEFAULT_SCHEME: u32 = 0;

#[derive(Debug, Clone)]
pub(crate) struct StoredScheme {
    pub scheme: ControlScheme,
    verbatim: Option<Vec<u8>>,
}

impl StoredScheme {
    pub fn full_xml(&self) -> Vec<u8> {
        match &self.verbatim {
            Some(bytes) => bytes.clone(),
            None => writer::write_full(&self.scheme).into_bytes(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct SchemeLibrary {
    by_index: HashMap<u32, StoredScheme>,
    by_device: HashMap<String, u32>,
}

impl SchemeLibrary {
    pub fn load(&mut self, index: u32, xml: &[u8]) -> Result<(), String> {
        let scheme = BMApplicationSchemeParser::new().parse(xml)?;
        // A game might want to send a scheme with nothing in it but the lib warns.
        // It is also how a truncated document looks like so this warning is
        // useful to debug why the controller would show nothing.
        if scheme.display_objects.is_empty() {
            log::warn!("control scheme {index} has no display objects");
        }
        self.by_index.insert(
            index,
            StoredScheme {
                scheme,
                verbatim: Some(xml.to_vec()),
            },
        );
        Ok(())
    }

    pub fn assign(&mut self, device: &str, index: u32) {
        self.by_device.insert(device.to_string(), index);
    }

    pub fn forget_device(&mut self, device: &str) {
        self.by_device.remove(device);
    }

    // Schemes are served per-device because one device might be in a
    // character selection screen while another might be playing already for example.
    pub fn for_device(&self, device: &str) -> Option<(u32, &StoredScheme)> {
        let index = self
            .by_device
            .get(device)
            .copied()
            .unwrap_or(DEFAULT_SCHEME);
        self.by_index.get(&index).map(|stored| (index, stored))
    }

    /// Every handler named by any loaded scheme. Registering these is what
    /// stops a button whose handler was never declared from doing nothing at
    /// all.
    pub fn button_handlers(&self) -> Vec<String> {
        let mut handlers: Vec<String> = self
            .by_index
            .values()
            .flat_map(|stored| &stored.scheme.display_objects)
            .map(|object| object.function_handler.clone())
            .filter(|handler| !handler.is_empty())
            .collect();
        handlers.sort();
        handlers.dedup();
        handlers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::externals::bm_registry_info::BMRegistryInfo;
    use crate::config::EngineConfig;
    use crate::devices::bm_address::BMAddress;
    use crate::devices::device_core::DeviceCore;
    use crate::engine::device_registry::DeviceRecord;
    use crate::engine::events::{Command, Event};
    use crate::engine::processing::Engine;
    use crate::types::device_type::DeviceType;

    const SCHEME: &[u8] = br#"<BMApplicationScheme width="480" height="320">
        <Layout><DisplayObject id="1" type="button" functionHandler="fire"/></Layout>
        </BMApplicationScheme>"#;
    const OTHER: &[u8] = br#"<BMApplicationScheme width="320" height="480">
        <Layout><DisplayObject id="1" type="button" functionHandler="jump"/></Layout>
        </BMApplicationScheme>"#;

    #[test]
    fn a_device_with_no_assignment_gets_the_default() {
        let mut lib = SchemeLibrary::default();
        lib.load(DEFAULT_SCHEME, SCHEME).unwrap();
        let (index, stored) = lib.for_device("phone").expect("served");
        assert_eq!(index, DEFAULT_SCHEME);
        assert_eq!(stored.scheme.width, 480);
    }

    #[test]
    fn an_assignment_picks_another_scheme() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME).unwrap();
        lib.load(1, OTHER).unwrap();
        lib.assign("tablet", 1);
        assert_eq!(lib.for_device("tablet").unwrap().1.scheme.width, 320);
        assert_eq!(lib.for_device("phone").unwrap().1.scheme.width, 480);
    }

    #[test]
    fn an_assignment_to_a_scheme_that_is_not_loaded_serves_nothing() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME).unwrap();
        lib.assign("tablet", 7);
        assert!(lib.for_device("tablet").is_none());
    }

    #[test]
    fn a_loaded_scheme_goes_back_out_exactly_as_it_came_in() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME).unwrap();
        assert_eq!(lib.for_device("phone").unwrap().1.full_xml(), SCHEME);
    }

    #[test]
    fn a_scheme_that_does_not_parse_is_rejected_and_changes_nothing() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME).unwrap();
        assert!(lib.load(0, br#"<BMApplicationScheme width=480>"#).is_err());
        assert_eq!(lib.for_device("phone").unwrap().1.scheme.width, 480);
    }

    /// A game is allowed to serve nothing, so this warns rather than refusing.
    #[test]
    fn a_scheme_with_no_objects_is_accepted() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, br#"<BMApplicationScheme width="480" height="320"/>"#)
            .unwrap();
        let (_, stored) = lib.for_device("phone").expect("served");
        assert!(stored.scheme.display_objects.is_empty());
        assert_eq!(stored.scheme.width, 480);
    }

    #[test]
    fn handlers_come_from_every_loaded_scheme() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME).unwrap();
        lib.load(1, OTHER).unwrap();
        assert_eq!(lib.button_handlers(), vec!["fire", "jump"]);
    }

    #[test]
    fn loading_the_same_index_replaces_it() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME).unwrap();
        lib.load(0, OTHER).unwrap();
        assert_eq!(lib.for_device("phone").unwrap().1.scheme.width, 320);
        assert_eq!(lib.button_handlers(), vec!["jump"]);
    }

    fn game_with(scheme: Option<&[u8]>) -> Engine {
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
        game.state.upsert_registry_info(BMRegistryInfo {
            slot_id: 0,
            app_id: "app".to_string(),
            current_players: None,
            max_players: None,
            device: DeviceCore::new(
                "phone".to_string(),
                "Phone".to_string(),
                DeviceType::Android,
            ),
            device_address: BMAddress::new("10.0.0.2".to_string(), 9080, 9081),
        });
        if let Some(xml) = scheme {
            game.emit(
                Command::LoadScheme {
                    index: 0,
                    xml: xml.to_vec(),
                },
                None,
            )
            .expect("the scheme parses");
        }
        game
    }

    /// What a controller puts on the wire when it asks for a layout.
    fn request_from(peer: &str) -> Vec<u8> {
        let mut phone = Engine::default();
        phone.init_local_device(DeviceCore::new(
            peer.to_string(),
            "Phone".to_string(),
            DeviceType::Android,
        ));
        phone.push_registry_update(DeviceRecord::new(
            DeviceCore::new("game".to_string(), "Game".to_string(), DeviceType::Unity),
            None,
        ));
        phone
            .make_request_xml("game", 320, 480, peer)
            .remove(0)
            .message()
            .to_vec()
    }

    #[test]
    fn a_game_holding_a_scheme_answers_the_request_itself() {
        let mut game = game_with(Some(SCHEME));
        let out = game.process_incoming(&request_from("phone"), &Default::default());

        assert!(
            !out.outgoings.is_empty(),
            "the scheme should have gone out without the game lifting a finger"
        );
        let answered = out.events.iter().any(|e| {
            matches!(
                e,
                Event::ControlSchemeRequested {
                    answered: true,
                    requester,
                    ..
                } if requester == "phone"
            )
        });
        assert!(answered, "the event should say it was answered");
    }

    #[test]
    fn a_game_holding_nothing_leaves_the_request_to_its_consumer() {
        let mut game = game_with(None);
        let out = game.process_incoming(&request_from("phone"), &Default::default());

        assert!(out.outgoings.is_empty(), "nothing to send, so nothing sent");
        let unanswered = out.events.iter().any(|e| {
            matches!(
                e,
                Event::ControlSchemeRequested {
                    answered: false,
                    ..
                }
            )
        });
        assert!(unanswered, "the consumer still has to hear about it");
    }

    #[test]
    fn the_chunk_size_is_the_callers_to_choose() {
        let scheme = format!(
            r#"<BMApplicationScheme width="480" height="320"><Layout>{}</Layout></BMApplicationScheme>"#,
            r#"<DisplayObject id="1" type="button" functionHandler="fire"/>"#.repeat(400)
        );

        let count = |chunk_bytes: u32| {
            let mut game = Engine::default();
            game.init_local_device(DeviceCore::new(
                "game".to_string(),
                "Game".to_string(),
                DeviceType::Unity,
            ));
            game.configure(EngineConfig {
                endpoint: Some(crate::policy::EndpointMode::Game),
                opens_sessions: false,
                max_chunk_bytes: chunk_bytes,
                ..Default::default()
            })
            .expect("the size is allowed");
            game.state.upsert_registry_info(BMRegistryInfo {
                slot_id: 0,
                app_id: "app".to_string(),
                current_players: None,
                max_players: None,
                device: DeviceCore::new(
                    "phone".to_string(),
                    "Phone".to_string(),
                    DeviceType::Android,
                ),
                device_address: BMAddress::new("10.0.0.2".to_string(), 9080, 9081),
            });
            game.make_byte_chunks("phone", "testXML", scheme.as_bytes())
                .len()
        };

        let small = count(1024);
        let default = count(crate::config::DEFAULT_MAX_CHUNK_BYTES);
        assert!(small > default, "{small} should beat {default}");
        assert_eq!(default, 1, "the default swallows a scheme this size whole");
    }

    #[test]
    fn a_chunk_at_the_ceiling_fits_the_smallest_known_peer_buffer() {
        let ceiling = (1..=200_000u32)
            .rev()
            .find(|n| {
                EngineConfig {
                    max_chunk_bytes: *n,
                    ..Default::default()
                }
                .check()
                .is_ok()
            })
            .expect("some size is allowed");

        let mut game = Engine::default();
        game.init_local_device(DeviceCore::new(
            "a-game-with-a-long-identifier".to_string(),
            "A Game With A Long Name".to_string(),
            DeviceType::Unity,
        ));
        game.configure(EngineConfig {
            endpoint: Some(crate::policy::EndpointMode::Game),
            opens_sessions: false,
            max_chunk_bytes: ceiling,
            ..Default::default()
        })
        .expect("the ceiling is allowed");
        game.state.upsert_registry_info(BMRegistryInfo {
            slot_id: 0,
            app_id: "app".to_string(),
            current_players: None,
            max_players: None,
            device: DeviceCore::new(
                "phone".to_string(),
                "Phone".to_string(),
                DeviceType::Android,
            ),
            device_address: BMAddress::new("10.0.0.2".to_string(), 9080, 9081),
        });

        let blob = vec![b'x'; ceiling as usize];
        let packets = game.make_byte_chunks("phone", "updateXML", &blob);
        assert_eq!(packets.len(), 1, "one chunk exactly at the ceiling");
        let framed = packets[0].payload.len();
        assert!(
            framed < 98304,
            "a chunk at the ceiling frames to {framed} bytes, past what a peer is known to read"
        );
    }

    #[test]
    fn a_game_can_introduce_itself_before_being_asked() {
        let mut game = game_with(None);
        let out = game
            .emit(
                Command::Introduce {
                    target: "phone".to_string(),
                },
                None,
            )
            .expect("a known peer");
        assert_eq!(out.outgoings.len(), 1, "the ack should have gone out");

        // Having introduced itself, it must not ack again when pinged.
        let ping = {
            let mut phone = Engine::default();
            phone.init_local_device(DeviceCore::new(
                "phone".to_string(),
                "Phone".to_string(),
                DeviceType::Android,
            ));
            phone.push_registry_update(DeviceRecord::new(
                DeviceCore::new("game".to_string(), "Game".to_string(), DeviceType::Unity),
                None,
            ));
            phone.make_ping_packet("game").remove(0).message().to_vec()
        };
        let pinged = game.process_incoming(&ping, &Default::default());
        assert!(
            pinged.outgoings.is_empty(),
            "the ping should not draw a second ack"
        );
    }

    #[test]
    fn a_connect_request_leaves_the_device_addressable() {
        let mut game = game_with(None);
        let request = {
            let mut server = Engine::default();
            server.init_local_device(DeviceCore::new(
                "reg".to_string(),
                "Registry".to_string(),
                DeviceType::Server,
            ));
            server.push_registry_update(DeviceRecord::new(
                DeviceCore::new("game".to_string(), "Game".to_string(), DeviceType::Unity),
                None,
            ));
            let info = BMRegistryInfo {
                slot_id: 0,
                app_id: "app".to_string(),
                current_players: None,
                max_players: None,
                device: DeviceCore::new(
                    "tablet".to_string(),
                    "Tablet".to_string(),
                    DeviceType::Android,
                ),
                device_address: BMAddress::new("10.0.0.9".to_string(), 9080, 9081),
            };
            server
                .make_message_invoke(
                    "game",
                    crate::engine::methods::DEVICE_CONNECT_REQUESTED,
                    None,
                    vec![crate::codec::messages::bm_encoding::Value::Object(
                        crate::codec::object::Object::BMRegistryInfo(info),
                    )],
                )
                .remove(0)
                .message()
                .to_vec()
        };

        game.process_incoming(&request, &Default::default());
        let out = game
            .emit(
                Command::Introduce {
                    target: "tablet".to_string(),
                },
                None,
            )
            .expect("the device was named to us");
        assert_eq!(
            out.outgoings.len(),
            1,
            "a device we were told about should be addressable"
        );
    }

    #[test]
    fn a_chunk_size_of_nothing_is_refused() {
        let mut game = Engine::default();
        assert!(
            game.configure(EngineConfig {
                max_chunk_bytes: 0,
                ..Default::default()
            })
            .is_err()
        );
    }

    #[test]
    fn a_scheme_names_its_handlers_the_moment_it_is_loaded() {
        let game = game_with(Some(SCHEME));
        assert!(
            game.game_policy.button_handlers.contains("fire"),
            "a button whose handler was never registered arrives as silence"
        );
    }
}
