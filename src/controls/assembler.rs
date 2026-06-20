// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::controls::parser::BMApplicationSchemeParser;
use crate::controls::{CONTROL_SCHEME_SET_ID, ControlScheme, UPDATE_SCHEME_SET_ID, merge};
use prost::Message;

#[derive(Debug, Clone)]
pub struct SchemeUpdate {
    pub scheme: Vec<u8>,
    pub initial: bool,
}

#[derive(Debug)]
pub enum SchemeOffer {
    Updated(SchemeUpdate),
    Consumed,
    NotScheme,
}

/// Turns completed control scheme chunk sets into a current, merged scheme.
/// Owns the scheme set id semantics and the parse/merge/dedup state so the
/// consumer never touches them. Independent of the BMEngine.
#[derive(Default)]
pub struct SchemeAssembler {
    base: Option<ControlScheme>,
    last_update: Option<Vec<u8>>,
    pending: Vec<Vec<u8>>,
}

impl SchemeAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn offer(&mut self, set_id: &str, blob: &[u8]) -> SchemeOffer {
        match set_id {
            CONTROL_SCHEME_SET_ID => self.apply_initial(blob),
            UPDATE_SCHEME_SET_ID => self.apply_update(blob),
            _ => SchemeOffer::NotScheme,
        }
    }

    pub fn current(&self) -> Option<Vec<u8>> {
        self.base.as_ref().map(encode)
    }

    pub fn reset(&mut self) {
        self.base = None;
        self.last_update = None;
        self.pending.clear();
    }

    fn apply_initial(&mut self, blob: &[u8]) -> SchemeOffer {
        let Some(scheme) = parse(blob) else {
            log::warn!("control scheme parse failed");
            return SchemeOffer::Consumed;
        };
        self.base = Some(scheme);
        self.last_update = None;
        // Replay any updates that arrived before the base.
        let pending = std::mem::take(&mut self.pending);
        for blob in pending {
            self.merge_into_base(&blob);
        }
        SchemeOffer::Updated(SchemeUpdate {
            scheme: self.current().unwrap_or_default(),
            initial: true,
        })
    }

    fn apply_update(&mut self, blob: &[u8]) -> SchemeOffer {
        if self.base.is_none() {
            self.pending.push(blob.to_vec());
            return SchemeOffer::Consumed;
        }
        if self.last_update.as_deref() == Some(blob) {
            return SchemeOffer::Consumed;
        }
        self.last_update = Some(blob.to_vec());
        if !self.merge_into_base(blob) {
            return SchemeOffer::Consumed;
        }
        SchemeOffer::Updated(SchemeUpdate {
            scheme: self.current().unwrap_or_default(),
            initial: false,
        })
    }

    fn merge_into_base(&mut self, blob: &[u8]) -> bool {
        let Some(update) = parse(blob) else {
            log::warn!("control scheme update parse failed");
            return false;
        };
        match self.base.as_mut() {
            Some(base) => {
                merge::apply_update(base, update);
                true
            }
            None => false,
        }
    }
}

fn parse(blob: &[u8]) -> Option<ControlScheme> {
    BMApplicationSchemeParser::new().parse(blob).ok()
}

fn encode(scheme: &ControlScheme) -> Vec<u8> {
    let mut buf = Vec::with_capacity(scheme.encoded_len());
    let _ = scheme.encode(&mut buf);
    buf
}

#[cfg(test)]
mod tests {
    use super::{SchemeAssembler, SchemeOffer};
    use crate::controls::ControlScheme;
    use prost::Message;

    const TEST_XML: &[u8] = br#"<BMApplicationScheme orientation="landscape" width="480" height="320"><DisplayObject id="1" type="button"/></BMApplicationScheme>"#;
    const UPDATE_XML: &[u8] =
        br#"<BMApplicationScheme><DisplayObject id="2" type="dpad"/></BMApplicationScheme>"#;

    fn decode(bytes: &[u8]) -> ControlScheme {
        ControlScheme::decode(bytes).unwrap()
    }

    #[test]
    fn initial_scheme_sets_base() {
        let mut a = SchemeAssembler::new();
        let SchemeOffer::Updated(u) = a.offer("testXML", TEST_XML) else {
            panic!("expected Updated");
        };
        assert!(u.initial);
        let s = decode(&u.scheme);
        assert_eq!(s.orientation, "landscape");
        assert_eq!(s.width, 480);
        assert_eq!(s.display_objects.len(), 1);
        assert_eq!(s.display_objects[0].id, 1);
    }

    #[test]
    fn update_replaces_objects_and_keeps_scalars() {
        let mut a = SchemeAssembler::new();
        a.offer("testXML", TEST_XML);
        let SchemeOffer::Updated(u) = a.offer("updateXML", UPDATE_XML) else {
            panic!("expected Updated");
        };
        assert!(!u.initial);
        let s = decode(&u.scheme);
        // Objects replaced by the update's set; scalars kept from the initial.
        assert_eq!(s.display_objects.len(), 1);
        assert_eq!(s.display_objects[0].id, 2);
        assert_eq!(s.orientation, "landscape");
        assert_eq!(s.width, 480);
    }

    #[test]
    fn update_before_base_is_buffered_then_replayed() {
        let mut a = SchemeAssembler::new();
        assert!(matches!(
            a.offer("updateXML", UPDATE_XML),
            SchemeOffer::Consumed
        ));
        let SchemeOffer::Updated(u) = a.offer("testXML", TEST_XML) else {
            panic!("expected Updated");
        };
        assert!(u.initial);
        let s = decode(&u.scheme);
        // The buffered update was applied on top of the base.
        assert_eq!(s.display_objects.len(), 1);
        assert_eq!(s.display_objects[0].id, 2);
        assert_eq!(s.orientation, "landscape");
    }

    #[test]
    fn duplicate_update_is_deduped() {
        let mut a = SchemeAssembler::new();
        a.offer("testXML", TEST_XML);
        assert!(matches!(
            a.offer("updateXML", UPDATE_XML),
            SchemeOffer::Updated(_)
        ));
        assert!(matches!(
            a.offer("updateXML", UPDATE_XML),
            SchemeOffer::Consumed
        ));
    }

    #[test]
    fn non_scheme_set_is_not_claimed() {
        let mut a = SchemeAssembler::new();
        assert!(matches!(
            a.offer("highScores", b"arbitrary"),
            SchemeOffer::NotScheme
        ));
    }

    #[test]
    fn reset_clears_state() {
        let mut a = SchemeAssembler::new();
        a.offer("testXML", TEST_XML);
        assert!(a.current().is_some());
        a.reset();
        assert!(a.current().is_none());
    }
}
