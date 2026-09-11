// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

//! The schemes a game holds, and which device gets which.
//!
//! A host serves several control schemes, one per kind of device, and picks
//! between them when a controller asks. Keeping them here is what lets the
//! engine answer `RequestXML` on the game's behalf, so a game never marshals a
//! scheme at request time.

use crate::controls::builder::SchemeBuilder;
use crate::controls::parser::BMApplicationSchemeParser;
use crate::controls::{Screen, writer};
use std::collections::HashMap;

pub(crate) const DEFAULT_SCHEME: u32 = 0;

#[derive(Debug, Clone)]
pub(crate) struct StoredScheme {
    builder: SchemeBuilder,
    verbatim: Option<Vec<u8>>,
    for_screen: Option<Screen>,
}

impl StoredScheme {
    #[cfg(test)]
    pub fn scheme(&self) -> &crate::controls::ControlScheme {
        self.builder.scheme()
    }

    #[cfg(test)]
    pub fn full_xml(&self) -> Vec<u8> {
        match &self.verbatim {
            Some(bytes) => bytes.clone(),
            None => writer::write_full(self.builder.scheme()).into_bytes(),
        }
    }

    /// The whole scheme, and nothing is waiting to be sent any more.
    ///
    /// A full document carries every resource, so it clears the marks exactly
    /// as an update does. Otherwise the first update after one would resend
    /// artwork the controller already has.
    pub fn take_full(&mut self) -> Vec<u8> {
        let xml = match &self.verbatim {
            Some(bytes) => bytes.clone(),
            None => writer::write_full(self.builder.scheme()).into_bytes(),
        };
        self.builder.clear_changed();
        xml
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct SchemeLibrary {
    by_index: HashMap<u32, StoredScheme>,
    by_device: HashMap<String, u32>,
}

impl SchemeLibrary {
    pub fn load(
        &mut self,
        index: u32,
        xml: &[u8],
        for_screen: Option<Screen>,
    ) -> Result<(), String> {
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
                builder: SchemeBuilder::from_scheme(scheme),
                verbatim: Some(xml.to_vec()),
                for_screen,
            },
        );
        Ok(())
    }

    /// Puts an empty scheme at an index, replacing whatever was there.
    pub fn begin(&mut self, index: u32, builder: SchemeBuilder, for_screen: Option<Screen>) {
        self.by_index.insert(
            index,
            StoredScheme {
                builder,
                verbatim: None,
                for_screen,
            },
        );
    }

    /// Which scheme answers a request from a screen of this size.
    ///
    /// In order: a scheme declared for exactly this screen, then whatever the
    /// game assigned this device, then the one whose shape suits the screen
    /// best, then index 0.
    pub fn for_request(
        &self,
        device: &str,
        width: i32,
        height: i32,
    ) -> Option<(u32, &StoredScheme)> {
        let asked = Screen { width, height };
        let declared = self
            .by_index
            .iter()
            .filter(|(_, stored)| stored.for_screen == Some(asked))
            .min_by_key(|(index, _)| **index);
        if let Some((index, stored)) = declared {
            return Some((*index, stored));
        }

        // A game that named a scheme for this device meant it, so nothing is
        // inferred over the top of it.
        if let Some(&index) = self.by_device.get(device)
            && let Some(stored) = self.by_index.get(&index)
        {
            return Some((index, stored));
        }

        self.best_shape_for(width, height).or_else(|| {
            self.by_index
                .get(&DEFAULT_SCHEME)
                .map(|s| (DEFAULT_SCHEME, s))
        })
    }

    /// The scheme whose shape leaves the least of this screen unused.
    ///
    /// A controller scales a scheme to fill the screen and keeps its aspect, so
    /// what a mismatch costs is bars down the sides rather than a broken
    /// layout. Comparing the long side against the short one is what decides
    /// that, and it makes the answer the same whichever way the phone reports
    /// itself.
    fn best_shape_for(&self, width: i32, height: i32) -> Option<(u32, &StoredScheme)> {
        let wanted = aspect(width, height)?;
        let mut best: Option<(u32, &StoredScheme, f32)> = None;
        for (&index, stored) in &self.by_index {
            let design = stored.builder.scheme();
            let Some(theirs) = aspect(design.width, design.height) else {
                continue;
            };
            let off = (theirs - wanted).abs();
            let better = match best {
                None => true,
                // Lowest index breaks a tie, so the answer never depends on
                // how a hash map felt like ordering itself today.
                Some((best_index, _, best_off)) => {
                    off < best_off || (off == best_off && index < best_index)
                }
            };
            if better {
                best = Some((index, stored, off));
            }
        }
        best.map(|(index, stored, _)| (index, stored))
    }

    /// The scheme that answers a request, ready to go and no longer waiting.
    pub fn take_full_for_request(
        &mut self,
        device: &str,
        width: i32,
        height: i32,
    ) -> Option<(u32, Vec<u8>)> {
        let index = self.for_request(device, width, height).map(|(i, _)| i)?;
        // Remembered, because the engine may have chosen this rather than the
        // game, and an update has to reach the scheme the device is holding.
        self.by_device.insert(device.to_string(), index);
        self.by_index
            .get_mut(&index)
            .map(|stored| (index, stored.take_full()))
    }

    /// A scheme to change, which is also what drops the bytes it arrived as.
    pub fn edit(&mut self, index: u32) -> Result<&mut SchemeBuilder, String> {
        let stored = self
            .by_index
            .get_mut(&index)
            .ok_or_else(|| format!("no scheme at index {index}"))?;
        stored.verbatim = None;
        Ok(&mut stored.builder)
    }

    /// The update for a scheme, after which nothing is marked changed any more.
    ///
    /// Taking it and clearing the marks is one step on purpose: artwork left
    /// marked would ride every update that followed, and the asymmetry that
    /// makes updates cheap would quietly disappear.
    pub fn take_update(&mut self, index: u32) -> Result<Vec<u8>, String> {
        let stored = self
            .by_index
            .get_mut(&index)
            .ok_or_else(|| format!("no scheme at index {index}"))?;
        let xml = writer::write_update(stored.builder.scheme()).into_bytes();
        stored.builder.clear_changed();
        Ok(xml)
    }

    /// Which scheme a device is being served, so an update goes to the one it
    /// is actually holding.
    pub fn index_for_device(&self, device: &str) -> Option<u32> {
        self.for_device(device).map(|(index, _)| index)
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
            .flat_map(|stored| &stored.builder.scheme().display_objects)
            .map(|object| object.function_handler.clone())
            .filter(|handler| !handler.is_empty())
            .collect();
        handlers.sort();
        handlers.dedup();
        handlers
    }
}

/// The long side over the short one, so a scheme and a screen can be compared
/// without caring which way either is held.
fn aspect(width: i32, height: i32) -> Option<f32> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let (long, short) = if width > height {
        (width, height)
    } else {
        (height, width)
    };
    Some(long as f32 / short as f32)
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

    const NAMED: &[u8] = br#"<BMApplicationScheme width="480" height="320">
        <Layout><DisplayObject id="1" name="fire" type="button" functionHandler="fire"/></Layout>
        </BMApplicationScheme>"#;

    fn built() -> SchemeBuilder {
        let mut b = SchemeBuilder::new(480, 320, "landscape", true, false, "linear");
        b.add_button(
            "fire",
            "onFire",
            crate::controls::builder::Rect::new(0.0, 0.0, 100.0, 100.0),
            b"up-art",
            b"down-art",
        )
        .unwrap();
        b
    }

    /// The stored bytes are an optimisation for a document nobody has touched.
    /// Once one has been edited they describe a layout that no longer exists,
    /// and serving them would answer every later request with the old one.
    #[test]
    fn editing_a_scheme_stops_it_being_served_as_the_bytes_it_arrived_as() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, NAMED, None).unwrap();
        assert_eq!(lib.for_device("phone").unwrap().1.full_xml(), NAMED);

        lib.edit(0).unwrap().set_hidden("fire", true).unwrap();

        let served = lib.for_device("phone").unwrap().1.full_xml();
        assert_ne!(served, NAMED);
        assert!(String::from_utf8_lossy(&served).contains(r#"hidden="yes""#));
    }

    #[test]
    fn editing_a_scheme_that_is_not_there_says_so_rather_than_creating_one() {
        let mut lib = SchemeLibrary::default();
        assert!(lib.edit(3).is_err());
        assert!(lib.for_device("phone").is_none());
    }

    /// Artwork rides one update and no more. If the marks survived, every
    /// update after a single replacement would carry the picture again and the
    /// form would cost the same as a full scheme.
    #[test]
    fn artwork_marked_changed_goes_out_once() {
        let mut lib = SchemeLibrary::default();
        lib.begin(0, built(), None);
        lib.edit(0)
            .unwrap()
            .replace_artwork("fire", "up", b"new-art")
            .unwrap();

        let first = lib.take_update(0).unwrap();
        assert!(String::from_utf8_lossy(&first).contains("<Resource "));

        let second = lib.take_update(0).unwrap();
        assert!(
            !String::from_utf8_lossy(&second).contains("<Resource "),
            "the layout still goes, the picture does not"
        );
    }

    #[test]
    fn a_built_scheme_is_served_like_a_loaded_one() {
        let mut lib = SchemeLibrary::default();
        lib.begin(0, built(), None);
        let served = lib.for_device("phone").unwrap().1.full_xml();
        assert!(String::from_utf8_lossy(&served).contains(r#"name="fire""#));
        assert_eq!(lib.button_handlers(), vec!["onFire"]);
    }

    /// The built path end to end. A game describes a layout in commands, the
    /// handlers it names become dispatchable with nothing having parsed a
    /// document, and the update goes out under the set id that merges.
    #[test]
    fn a_scheme_built_by_command_answers_requests_and_sends_updates() {
        use crate::controls::builder::Rect;

        let mut game = game_with(None);
        game.emit(
            Command::BeginScheme {
                index: 0,
                width: 480,
                height: 320,
                orientation: "landscape".to_string(),
                touch_enabled: true,
                accelerometer_enabled: false,
                sample: "linear".to_string(),
                for_screen: None,
            },
            None,
        )
        .expect("a scheme can be started");
        game.emit(
            Command::AddButton {
                index: 0,
                name: "fire".to_string(),
                handler: "onFire".to_string(),
                rect: Rect::new(340.0, 30.0, 120.0, 70.0),
                up: b"up-art".to_vec(),
                down: b"down-art".to_vec(),
            },
            None,
        )
        .expect("a button can be added");

        assert!(
            game.game_policy.button_handlers.contains("onFire"),
            "registered as it was built, not when a document was read"
        );

        let full = game
            .process_incoming(&request_from("phone"), &Default::default())
            .outgoings;
        assert_eq!(
            chunk_set_id(&full[0]),
            crate::controls::CONTROL_SCHEME_SET_ID,
            "a request is answered from what was built"
        );

        let update = game
            .emit(
                Command::SendSchemeUpdate {
                    target: "phone".to_string(),
                    index: None,
                },
                None,
            )
            .expect("a known peer")
            .outgoings;
        assert_eq!(
            chunk_set_id(&update[0]),
            crate::controls::UPDATE_SCHEME_SET_ID
        );
    }

    #[test]
    fn an_update_to_a_device_holding_no_scheme_is_refused() {
        let mut game = game_with(None);
        assert!(
            game.emit(
                Command::SendSchemeUpdate {
                    target: "phone".to_string(),
                    index: None,
                },
                None,
            )
            .is_err()
        );
    }

    /// A scheme authored for the screen that is asking beats one merely
    /// assigned, because it was written for that device rather than chosen for
    /// it. The design size in the document has no say: this scheme is authored
    /// at 480x320 and declared for a phone reporting 1080x2151.
    #[test]
    fn a_scheme_declared_for_this_screen_wins_over_an_assignment() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME, None).unwrap();
        lib.load(1, OTHER, None).unwrap();
        lib.load(
            2,
            NAMED,
            Some(Screen {
                width: 1080,
                height: 2151,
            }),
        )
        .unwrap();
        lib.assign("phone", 1);

        assert_eq!(lib.for_request("phone", 1080, 2151).unwrap().0, 2);
        assert_eq!(
            lib.for_request("phone", 800, 600).unwrap().0,
            1,
            "no screen matches, so the assignment stands"
        );
        assert_eq!(
            lib.for_request("tablet", 800, 600).unwrap().0,
            DEFAULT_SCHEME,
            "and with neither, index 0"
        );
    }

    #[test]
    fn a_game_declaring_no_screens_is_unaffected_by_matching() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME, None).unwrap();
        assert_eq!(lib.for_request("phone", 1080, 2151).unwrap().0, 0);
    }

    /// Schemes belong to the game, not to one controller's session, so the
    /// reset that ends a session must leave them alone.
    #[test]
    fn ending_a_session_does_not_cost_the_game_its_schemes() {
        let mut game = game_with(Some(SCHEME));
        game.reset_game_session();
        assert!(game.schemes.for_device("phone").is_some());
        assert_eq!(game.schemes.button_handlers(), vec!["fire"]);
    }

    /// Switching a connected device to another scheme sends that scheme as an
    /// update, and the device has never seen its artwork. So a scheme nobody
    /// has been served carries all of it, and only stops once a document has
    /// actually gone out.
    #[test]
    fn a_scheme_nobody_has_seen_sends_all_of_its_artwork() {
        let mut lib = SchemeLibrary::default();
        lib.begin(1, built(), None);

        let first = lib.take_update(1).unwrap();
        assert!(
            String::from_utf8_lossy(&first).contains("<Resource "),
            "a device switching to this scheme has none of its pictures"
        );

        let second = lib.take_update(1).unwrap();
        assert!(!String::from_utf8_lossy(&second).contains("<Resource "));
    }

    /// Answering a request carries every resource, so it settles the same debt
    /// an update would and the next update is layout alone.
    #[test]
    fn serving_a_whole_scheme_leaves_nothing_waiting() {
        let mut lib = SchemeLibrary::default();
        lib.begin(0, built(), None);

        let (_, full) = lib
            .take_full_for_request("phone", 800, 600)
            .expect("served");
        assert!(String::from_utf8_lossy(&full).contains("<Resource "));

        let update = lib.take_update(0).unwrap();
        assert!(
            !String::from_utf8_lossy(&update).contains("<Resource "),
            "the controller already has every picture"
        );
    }

    const WIDE: &[u8] = br#"<BMApplicationScheme width="568" height="320">
        <Layout><DisplayObject id="1" name="fire" type="button" functionHandler="fire"/></Layout>
        </BMApplicationScheme>"#;

    /// The case a shipped game got wrong. It matched one handset exactly and
    /// gave every other widescreen phone the narrow scheme, because nothing
    /// compared the shapes.
    #[test]
    fn a_screen_gets_the_scheme_shaped_most_like_it() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME, None).unwrap(); // 480x320, 1.50
        lib.load(1, WIDE, None).unwrap(); // 568x320, 1.775

        // A modern handset, near 2:1 either way up.
        assert_eq!(lib.for_request("tall", 1080, 2151).unwrap().0, 1);
        assert_eq!(lib.for_request("tall", 2151, 1080).unwrap().0, 1);
        // Something closer to three by two.
        assert_eq!(lib.for_request("squat", 1024, 683).unwrap().0, 0);
    }

    #[test]
    fn a_game_with_one_scheme_is_unaffected_by_any_of_it() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME, None).unwrap();
        assert_eq!(lib.for_request("phone", 1080, 2151).unwrap().0, 0);
    }

    /// A game naming a scheme for a device meant it, so nothing is inferred
    /// over the top. Only a screen declared outright outranks it.
    #[test]
    fn what_a_game_assigned_beats_what_the_shape_suggests() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME, None).unwrap();
        lib.load(1, WIDE, None).unwrap();
        lib.assign("phone", 0);

        assert_eq!(lib.for_request("phone", 1080, 2151).unwrap().0, 0);

        lib.load(
            2,
            WIDE,
            Some(Screen {
                width: 1080,
                height: 2151,
            }),
        )
        .unwrap();
        assert_eq!(lib.for_request("phone", 1080, 2151).unwrap().0, 2);
    }

    /// Whatever was served is what the device is holding, so an update with no
    /// index named has to find its way back to the same one.
    #[test]
    fn a_scheme_chosen_for_a_device_is_the_one_its_updates_reach() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME, None).unwrap();
        lib.load(1, WIDE, None).unwrap();
        assert_eq!(lib.index_for_device("tall"), Some(0), "nothing served yet");

        lib.take_full_for_request("tall", 1080, 2151)
            .expect("served");
        assert_eq!(lib.index_for_device("tall"), Some(1));
    }

    #[test]
    fn serving_says_which_scheme_went() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME, None).unwrap();
        lib.load(1, WIDE, None).unwrap();

        let (index, _) = lib
            .take_full_for_request("tall", 1080, 2151)
            .expect("served");
        assert_eq!(index, 1, "a game has to know which one to update later");
    }

    #[test]
    fn a_screen_of_no_size_falls_back_rather_than_dividing_by_it() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME, None).unwrap();
        lib.load(1, WIDE, None).unwrap();
        assert_eq!(lib.for_request("odd", 0, 0).unwrap().0, DEFAULT_SCHEME);
    }

    #[test]
    fn a_device_with_no_assignment_gets_the_default() {
        let mut lib = SchemeLibrary::default();
        lib.load(DEFAULT_SCHEME, SCHEME, None).unwrap();
        let (index, stored) = lib.for_device("phone").expect("served");
        assert_eq!(index, DEFAULT_SCHEME);
        assert_eq!(stored.scheme().width, 480);
    }

    #[test]
    fn an_assignment_picks_another_scheme() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME, None).unwrap();
        lib.load(1, OTHER, None).unwrap();
        lib.assign("tablet", 1);
        assert_eq!(lib.for_device("tablet").unwrap().1.scheme().width, 320);
        assert_eq!(lib.for_device("phone").unwrap().1.scheme().width, 480);
    }

    #[test]
    fn an_assignment_to_a_scheme_that_is_not_loaded_serves_nothing() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME, None).unwrap();
        lib.assign("tablet", 7);
        assert!(lib.for_device("tablet").is_none());
    }

    #[test]
    fn a_loaded_scheme_goes_back_out_exactly_as_it_came_in() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME, None).unwrap();
        assert_eq!(lib.for_device("phone").unwrap().1.full_xml(), SCHEME);
    }

    #[test]
    fn a_scheme_that_does_not_parse_is_rejected_and_changes_nothing() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME, None).unwrap();
        assert!(
            lib.load(0, br#"<BMApplicationScheme width=480>"#, None)
                .is_err()
        );
        assert_eq!(lib.for_device("phone").unwrap().1.scheme().width, 480);
    }

    /// A game is allowed to serve nothing, so this warns rather than refusing.
    #[test]
    fn a_scheme_with_no_objects_is_accepted() {
        let mut lib = SchemeLibrary::default();
        lib.load(
            0,
            br#"<BMApplicationScheme width="480" height="320"/>"#,
            None,
        )
        .unwrap();
        let (_, stored) = lib.for_device("phone").expect("served");
        assert!(stored.scheme().display_objects.is_empty());
        assert_eq!(stored.scheme().width, 480);
    }

    #[test]
    fn handlers_come_from_every_loaded_scheme() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME, None).unwrap();
        lib.load(1, OTHER, None).unwrap();
        assert_eq!(lib.button_handlers(), vec!["fire", "jump"]);
    }

    #[test]
    fn loading_the_same_index_replaces_it() {
        let mut lib = SchemeLibrary::default();
        lib.load(0, SCHEME, None).unwrap();
        lib.load(0, OTHER, None).unwrap();
        assert_eq!(lib.for_device("phone").unwrap().1.scheme().width, 320);
        assert_eq!(lib.button_handlers(), vec!["jump"]);
    }

    fn phone_core() -> DeviceCore {
        let mut core = DeviceCore::new(
            "phone".to_string(),
            "Phone".to_string(),
            DeviceType::Android,
        );
        core.address = Some(BMAddress::new("10.0.0.2".to_string(), 9080, 9081));
        core
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
                    for_screen: None,
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

    /// The set id a chunk went out under, which is the only thing telling a
    /// controller whether to replace its layout or merge into it.
    fn chunk_set_id(outgoing: &crate::engine::events::Outgoing) -> String {
        let mut pkt = crate::codec::externals::bm_packet::BMPacket::default();
        crate::engine::protocol::deserialize_message(outgoing.message(), &mut pkt)
            .expect("an outgoing holds a message");
        let msg = pkt.message.expect("and the message holds an object");
        let mut cur = crate::codec::bm_stream::BMStream::view(msg.as_slice());
        match crate::codec::object::Object::decode(&mut cur).expect("which decodes") {
            crate::codec::object::Object::BMByteChunk(chunk) => chunk.set_id,
            other => panic!("expected a byte chunk, got {other:?}"),
        }
    }

    /// A game answering a request sends a whole scheme; a game changing the
    /// layout mid play sends an update. The two are the same transport and
    /// differ only by the set id, so nothing else distinguishes them.
    #[test]
    fn an_update_goes_out_under_its_own_set_id() {
        let mut game = game_with(Some(SCHEME));

        let full = game
            .process_incoming(&request_from("phone"), &Default::default())
            .outgoings;
        assert_eq!(
            chunk_set_id(&full[0]),
            crate::controls::CONTROL_SCHEME_SET_ID
        );

        let update = game
            .emit(
                Command::UpdateScheme {
                    target: "phone".to_string(),
                    xml: OTHER.to_vec(),
                },
                None,
            )
            .expect("a known peer")
            .outgoings;
        assert_eq!(
            chunk_set_id(&update[0]),
            crate::controls::UPDATE_SCHEME_SET_ID
        );
    }

    /// An update introduces the buttons a game did not have before, so its
    /// handlers have to become dispatchable or the new controls arrive as
    /// silence.
    #[test]
    fn an_update_names_its_handlers_too() {
        let mut game = game_with(Some(SCHEME));
        assert!(!game.game_policy.button_handlers.contains("jump"));

        game.emit(
            Command::UpdateScheme {
                target: "phone".to_string(),
                xml: OTHER.to_vec(),
            },
            None,
        )
        .expect("a known peer");

        assert!(game.game_policy.button_handlers.contains("jump"));
        assert!(
            game.game_policy.button_handlers.contains("fire"),
            "and the ones already known are not lost"
        );
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
                Command::PeerReachable {
                    device: phone_core(),
                },
                None,
            )
            .expect("a peer we just named");
        assert!(!out.outgoings.is_empty(), "the ack should have gone out");

        // And saying it again must not repeat the introduction.
        let again = game
            .emit(
                Command::PeerReachable {
                    device: phone_core(),
                },
                None,
            )
            .expect("saying it twice is not an error");
        assert!(
            again.outgoings.is_empty(),
            "a second report should not draw a second ack"
        );

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
                Command::Vibrate {
                    target: "tablet".to_string(),
                },
                None,
            )
            .expect("a device we were told about should be addressable");
        assert!(!out.outgoings.is_empty());
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
