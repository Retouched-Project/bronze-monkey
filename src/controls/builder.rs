// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

//! Building a control scheme, the inverse of parsing one.
//!
//! A game describes its controls in design pixels and gets back a
//! `ControlScheme` the writer serialises in either form. Objects are addressed
//! by the name the caller chose; ids are ours, assigned here, and never asked
//! for, since making a game track numbers we handed it is bookkeeping for
//! nothing.

use crate::controls::parser::{DEFAULT_DEADZONE, DEFAULT_SAMPLING_MODE};
use crate::controls::{AppResource, ContextMenuOption, ControlAsset, ControlScheme, DisplayObject};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

const NULL_HANDLER: &str = "nullHandler";

pub const DPAD_STATES: [&str; 9] = [
    "inactive",
    "up",
    "down",
    "left",
    "right",
    "left_up",
    "left_down",
    "right_up",
    "right_down",
];

/// A rectangle in design pixels, which is what a game thinks in. Normalised to
/// the fractions the wire carries when the object is added.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(left: f32, top: f32, width: f32, height: f32) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }
}

pub struct SchemeBuilder {
    scheme: ControlScheme,
    by_content: HashMap<u64, Vec<i32>>,
    next_resource: i32,
    next_object: i32,
}

impl SchemeBuilder {
    pub fn new(
        width: i32,
        height: i32,
        orientation: &str,
        touch_enabled: bool,
        accelerometer_enabled: bool,
        sample: &str,
    ) -> Self {
        Self {
            scheme: ControlScheme {
                version: "0.1".to_string(),
                orientation: orientation.to_string(),
                touch_enabled,
                accelerometer_enabled,
                width,
                height,
                sample: sample.to_string(),
                ..Default::default()
            },
            by_content: HashMap::new(),
            next_resource: 1,
            next_object: 1,
        }
    }

    pub fn scheme(&self) -> &ControlScheme {
        &self.scheme
    }

    pub fn into_scheme(self) -> ControlScheme {
        self.scheme
    }

    /// A static picture. Takes no input, so it reports nothing.
    pub fn add_image(&mut self, name: &str, rect: Rect, artwork: &[u8]) -> Result<(), String> {
        let up = self.resource_id(artwork);
        self.add_object(name, "image", NULL_HANDLER, rect, vec![asset("up", up)])
    }

    /// A button with the two states the controller draws, reporting under
    /// `handler` when it is pressed and released.
    pub fn add_button(
        &mut self,
        name: &str,
        handler: &str,
        rect: Rect,
        up: &[u8],
        down: &[u8],
    ) -> Result<(), String> {
        let up_id = self.resource_id(up);
        let down_id = self.resource_id(down);
        self.add_object(
            name,
            "button",
            handler,
            rect,
            vec![asset("up", up_id), asset("down", down_id)],
        )
    }

    /// A dpad, its artwork given in `DPAD_STATES` order.
    ///
    /// `deadzone` is the fraction of the control ignored around the centre and
    /// `radial` asks for a wheel rather than eight sectors.
    pub fn add_dpad(
        &mut self,
        name: &str,
        handler: &str,
        rect: Rect,
        states: [&[u8]; 9],
        deadzone: f32,
        radial: bool,
    ) -> Result<(), String> {
        let assets = DPAD_STATES
            .iter()
            .zip(states)
            .map(|(state, artwork)| asset(state, self.resource_id(artwork)))
            .collect::<Vec<_>>();
        self.add_object(name, "dpad", handler, rect, assets)?;
        let object = self.object_mut(name)?;
        object.deadzone = deadzone;
        object.radial = radial;
        Ok(())
    }

    /// A line of text the game can change later with `set_text`.
    pub fn add_text(
        &mut self,
        name: &str,
        rect: Rect,
        text: &str,
        size: f32,
        color: i32,
    ) -> Result<(), String> {
        self.add_object(name, "text", NULL_HANDLER, rect, Vec::new())?;
        let height = self.scheme.height as f32;
        let object = self.object_mut(name)?;
        object.text = text.to_string();
        object.text_size = size / height;
        object.color = color;
        Ok(())
    }

    pub fn set_hit_rect(&mut self, name: &str, rect: Rect) -> Result<(), String> {
        let (width, height) = self.design_size();
        let object = self.object_mut(name)?;
        object.has_hit_rect = true;
        object.hit_left = rect.left / width;
        object.hit_top = rect.top / height;
        object.hit_width = rect.width / width;
        object.hit_height = rect.height / height;
        Ok(())
    }

    pub fn set_hidden(&mut self, name: &str, hidden: bool) -> Result<(), String> {
        self.object_mut(name)?.hidden = hidden;
        Ok(())
    }

    pub fn set_text(&mut self, name: &str, text: &str) -> Result<(), String> {
        self.object_mut(name)?.text = text.to_string();
        Ok(())
    }

    /// Points one of an object's assets at new artwork.
    ///
    /// The old resource is left alone and a fresh one takes its place, because
    /// artwork is shared: writing over it would change every other object using
    /// the same bytes. Only the new id is marked changed, so the next update
    /// carries this picture and no other.
    pub fn replace_artwork(
        &mut self,
        name: &str,
        asset_name: &str,
        artwork: &[u8],
    ) -> Result<(), String> {
        let id = self.resource_id(artwork);
        let object = self.object_mut(name)?;
        let slot = object
            .assets
            .iter_mut()
            .find(|a| a.name == asset_name)
            .ok_or_else(|| format!("object '{name}' has no asset '{asset_name}'"))?;
        slot.resource_ref = id;
        if !self.scheme.changed_resources.contains(&id) {
            self.scheme.changed_resources.push(id);
        }
        Ok(())
    }

    /// Drops an object. Its artwork stays, since ids are referenced by every
    /// other object and renumbering them to reclaim a picture would be a bigger
    /// change than the one asked for.
    pub fn remove(&mut self, name: &str) -> Result<(), String> {
        let before = self.scheme.display_objects.len();
        self.scheme.display_objects.retain(|o| o.name != name);
        if self.scheme.display_objects.len() == before {
            return Err(format!("no object named '{name}'"));
        }
        Ok(())
    }

    pub fn add_menu_option(&mut self, title: &str, event: &str, close_on_select: bool, icon: i32) {
        self.scheme.options.push(ContextMenuOption {
            icon_res_id: icon,
            title: title.to_string(),
            event: event.to_string(),
            close_on_select,
        });
    }

    /// Every handler the scheme names, which is what stops a button the game
    /// just added from arriving as silence.
    pub fn button_handlers(&self) -> Vec<String> {
        let mut handlers: Vec<String> = self
            .scheme
            .display_objects
            .iter()
            .map(|o| o.function_handler.clone())
            .filter(|h| !h.is_empty() && h != NULL_HANDLER)
            .collect();
        handlers.sort();
        handlers.dedup();
        handlers
    }

    /// Forgets which resources changed, once an update carrying them has gone.
    pub fn clear_changed(&mut self) {
        self.scheme.changed_resources.clear();
    }

    fn design_size(&self) -> (f32, f32) {
        (self.scheme.width as f32, self.scheme.height as f32)
    }

    fn add_object(
        &mut self,
        name: &str,
        kind: &str,
        handler: &str,
        rect: Rect,
        assets: Vec<ControlAsset>,
    ) -> Result<(), String> {
        if name.is_empty() {
            return Err("an object needs a name to be addressed by".to_string());
        }
        if self.scheme.display_objects.iter().any(|o| o.name == name) {
            return Err(format!("an object named '{name}' is already in the scheme"));
        }
        let (width, height) = self.design_size();
        let id = self.next_object;
        self.next_object += 1;
        self.scheme.display_objects.push(DisplayObject {
            id,
            r#type: kind.to_string(),
            name: name.to_string(),
            function_handler: handler.to_string(),
            left: rect.left / width,
            top: rect.top / height,
            width: rect.width / width,
            height: rect.height / height,
            hidden: false,
            sampling_mode: self.scheme.sample.clone(),
            deadzone: DEFAULT_DEADZONE,
            assets,
            ..Default::default()
        });
        Ok(())
    }

    fn object_mut(&mut self, name: &str) -> Result<&mut DisplayObject, String> {
        self.scheme
            .display_objects
            .iter_mut()
            .find(|o| o.name == name)
            .ok_or_else(|| format!("no object named '{name}'"))
    }

    /// The id for this artwork, adding it only if it is new.
    ///
    /// The hash picks candidates and the bytes decide, since a collision would
    /// otherwise make two different pictures silently the same one.
    fn resource_id(&mut self, artwork: &[u8]) -> i32 {
        let hash = hash_of(artwork);
        if let Some(ids) = self.by_content.get(&hash) {
            for id in ids {
                if self
                    .scheme
                    .resources
                    .iter()
                    .any(|r| r.id == *id && r.bitmap == artwork)
                {
                    return *id;
                }
            }
        }
        let id = self.next_resource;
        self.next_resource += 1;
        self.scheme.resources.push(AppResource {
            id,
            bitmap: artwork.to_vec(),
            r#type: "image".to_string(),
        });
        self.by_content.entry(hash).or_default().push(id);
        id
    }
}

fn asset(name: &str, resource_ref: i32) -> ControlAsset {
    ControlAsset {
        name: name.to_string(),
        resource_ref,
    }
}

fn hash_of(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

impl Default for SchemeBuilder {
    fn default() -> Self {
        Self::new(480, 320, "landscape", true, false, DEFAULT_SAMPLING_MODE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: &[u8] = b"red-png-bytes";
    const BLUE: &[u8] = b"blue-png-bytes";

    fn builder() -> SchemeBuilder {
        SchemeBuilder::new(480, 320, "landscape", true, false, "linear")
    }

    #[test]
    fn design_pixels_become_fractions_of_the_design_size() {
        let mut b = builder();
        b.add_image("bg", Rect::new(240.0, 160.0, 120.0, 80.0), RED)
            .unwrap();
        let object = &b.scheme().display_objects[0];
        assert_eq!(object.left, 0.5);
        assert_eq!(object.top, 0.5);
        assert_eq!(object.width, 0.25);
        assert_eq!(object.height, 0.25);
    }

    #[test]
    fn the_same_artwork_is_stored_once_however_many_objects_use_it() {
        let mut b = builder();
        b.add_button("a", "fire", Rect::new(0.0, 0.0, 10.0, 10.0), RED, BLUE)
            .unwrap();
        b.add_button("b", "jump", Rect::new(20.0, 0.0, 10.0, 10.0), RED, BLUE)
            .unwrap();
        assert_eq!(b.scheme().resources.len(), 2, "two pictures, four uses");
        let ids: Vec<i32> = b
            .scheme()
            .display_objects
            .iter()
            .flat_map(|o| &o.assets)
            .map(|a| a.resource_ref)
            .collect();
        assert_eq!(ids, vec![1, 2, 1, 2]);
    }

    #[test]
    fn an_update_costs_the_same_however_big_the_artwork_is() {
        fn update_for(artwork: &[u8]) -> usize {
            let mut b = builder();
            b.add_image("bg", Rect::new(0.0, 0.0, 480.0, 320.0), artwork)
                .unwrap();
            b.add_button(
                "fire",
                "onFire",
                Rect::new(340.0, 30.0, 120.0, 70.0),
                RED,
                BLUE,
            )
            .unwrap();
            b.set_hidden("fire", true).unwrap();
            crate::controls::writer::write_update(b.scheme()).len()
        }

        let small = update_for(RED);
        let large = update_for(&vec![0xA5; 600 * 1024]);
        assert_eq!(small, large);
    }

    #[test]
    fn a_duplicate_name_is_refused_rather_than_taken_as_the_first_match() {
        let mut b = builder();
        b.add_image("bg", Rect::new(0.0, 0.0, 10.0, 10.0), RED)
            .unwrap();
        let again = b.add_image("bg", Rect::new(0.0, 0.0, 10.0, 10.0), BLUE);
        assert!(again.is_err());
        assert_eq!(b.scheme().display_objects.len(), 1);
    }

    #[test]
    fn replacing_artwork_leaves_the_picture_other_objects_share() {
        let mut b = builder();
        b.add_image("one", Rect::new(0.0, 0.0, 10.0, 10.0), RED)
            .unwrap();
        b.add_image("two", Rect::new(20.0, 0.0, 10.0, 10.0), RED)
            .unwrap();
        assert_eq!(b.scheme().resources.len(), 1);

        b.replace_artwork("one", "up", BLUE).unwrap();

        let objects = &b.scheme().display_objects;
        assert_eq!(objects[0].assets[0].resource_ref, 2);
        assert_eq!(
            objects[1].assets[0].resource_ref, 1,
            "the other is untouched"
        );
        assert_eq!(b.scheme().changed_resources, vec![2]);
    }

    #[test]
    fn only_the_new_artwork_is_marked_changed() {
        let mut b = builder();
        b.add_image("one", Rect::new(0.0, 0.0, 10.0, 10.0), RED)
            .unwrap();
        assert!(
            b.scheme().changed_resources.is_empty(),
            "nothing changed yet"
        );

        b.replace_artwork("one", "up", BLUE).unwrap();
        assert_eq!(b.scheme().changed_resources, vec![2]);

        b.clear_changed();
        assert!(b.scheme().changed_resources.is_empty());
    }

    #[test]
    fn an_image_reports_nothing_and_a_button_reports_its_handler() {
        let mut b = builder();
        b.add_image("bg", Rect::new(0.0, 0.0, 10.0, 10.0), RED)
            .unwrap();
        b.add_button("fire", "onFire", Rect::new(0.0, 0.0, 10.0, 10.0), RED, BLUE)
            .unwrap();
        assert_eq!(b.scheme().display_objects[0].function_handler, NULL_HANDLER);
        assert_eq!(b.button_handlers(), vec!["onFire".to_string()]);
    }

    #[test]
    fn a_dpad_names_its_nine_states_the_way_the_wire_does() {
        let mut b = builder();
        let art: [&[u8]; 9] = [RED, BLUE, RED, BLUE, RED, BLUE, RED, BLUE, RED];
        b.add_dpad(
            "pad",
            "onPad",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            art,
            0.3,
            true,
        )
        .unwrap();
        let object = &b.scheme().display_objects[0];
        let names: Vec<&str> = object.assets.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, DPAD_STATES.to_vec());
        assert_eq!(object.deadzone, 0.3);
        assert!(object.radial);
    }

    #[test]
    fn text_size_is_normalised_like_every_other_measurement() {
        let mut b = builder();
        b.add_text(
            "score",
            Rect::new(0.0, 0.0, 100.0, 32.0),
            "0",
            32.0,
            0xFFFFFF,
        )
        .unwrap();
        let object = &b.scheme().display_objects[0];
        assert_eq!(object.text_size, 0.1, "32 of 320 design pixels");
        assert_eq!(object.text, "0");
    }

    #[test]
    fn addressing_an_object_that_is_not_there_says_so() {
        let mut b = builder();
        assert!(b.set_hidden("ghost", true).is_err());
        assert!(b.set_text("ghost", "hi").is_err());
        assert!(b.remove("ghost").is_err());
        assert!(b.replace_artwork("ghost", "up", RED).is_err());
    }

    /// The builder is only useful if the writer can serialise what it makes and
    /// the parser reads it back the same, since that is the round trip every
    /// scheme takes on its way to a controller.
    #[test]
    fn a_built_scheme_survives_being_written_and_parsed_back() {
        let mut b = builder();
        b.add_image("bg", Rect::new(0.0, 0.0, 480.0, 320.0), RED)
            .unwrap();
        b.add_button(
            "fire",
            "onFire",
            Rect::new(340.0, 30.0, 120.0, 70.0),
            RED,
            BLUE,
        )
        .unwrap();
        b.set_hit_rect("fire", Rect::new(330.0, 20.0, 140.0, 90.0))
            .unwrap();
        b.add_text(
            "score",
            Rect::new(10.0, 10.0, 100.0, 32.0),
            "0",
            32.0,
            0xFFFFFF,
        )
        .unwrap();
        b.add_menu_option("Quit", "quit", true, 0);
        let built = b.into_scheme();

        let xml = crate::controls::writer::write_full(&built);
        let back = crate::controls::parser::BMApplicationSchemeParser::new()
            .parse(xml.as_bytes())
            .expect("what we write, we can read");

        assert_eq!(back.width, built.width);
        assert_eq!(back.height, built.height);
        assert_eq!(back.orientation, built.orientation);
        assert_eq!(back.touch_enabled, built.touch_enabled);
        assert_eq!(back.resources.len(), built.resources.len());
        assert_eq!(back.options.len(), 1);
        assert_eq!(back.display_objects.len(), 3);
        for (was, now) in built.display_objects.iter().zip(&back.display_objects) {
            assert_eq!(now.name, was.name);
            assert_eq!(now.r#type, was.r#type);
            assert_eq!(now.function_handler, was.function_handler);
            assert_eq!(now.left, was.left);
            assert_eq!(now.width, was.width);
            assert_eq!(now.has_hit_rect, was.has_hit_rect);
            assert_eq!(now.assets.len(), was.assets.len());
        }
    }

    /// The asymmetry the update form exists for: an update carries the whole
    /// layout but only the picture that changed.
    #[test]
    fn an_update_from_a_built_scheme_carries_only_the_replaced_artwork() {
        let mut b = builder();
        b.add_image("bg", Rect::new(0.0, 0.0, 480.0, 320.0), RED)
            .unwrap();
        b.add_button(
            "fire",
            "onFire",
            Rect::new(340.0, 30.0, 120.0, 70.0),
            RED,
            BLUE,
        )
        .unwrap();
        b.replace_artwork("bg", "up", b"a-third-picture").unwrap();

        let update = crate::controls::writer::write_update(b.scheme());
        let back = crate::controls::parser::BMApplicationSchemeParser::new()
            .parse(update.as_bytes())
            .expect("an update parses like any document");

        assert_eq!(back.display_objects.len(), 2, "the whole layout goes");
        assert_eq!(back.resources.len(), 1, "but only the new picture");
        assert_eq!(back.resources[0].bitmap, b"a-third-picture");
    }

    #[test]
    fn an_asset_an_object_does_not_have_is_refused() {
        let mut b = builder();
        b.add_image("bg", Rect::new(0.0, 0.0, 10.0, 10.0), RED)
            .unwrap();
        assert!(
            b.replace_artwork("bg", "down", BLUE).is_err(),
            "an image has no down state"
        );
    }
}
