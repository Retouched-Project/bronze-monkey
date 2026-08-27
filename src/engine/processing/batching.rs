// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey
//
//! The pending touch set and when it goes on the wire.
//!
//! A caller reports what fingers did and the engine keeps the set they add up
//! to, sending it at the cadence the game asked for. Fingers are reported as
//! transitions, so unlike a sensor reading none can be dropped, but a batch of
//! them folds to the same set as the same batch applied one at a time, which is
//! what lets a caller hand them over at the boundary rather than one by one.

use super::Engine;
use crate::codec::externals::bm_reliability::BMReliability;
use crate::codec::messages::touch::Touch;
use crate::engine::events::{Outgoing, TouchEvent, TouchPhase};
use crate::types::channel_type::ChannelType;
use crate::types::touch_state::TouchState;
use std::collections::BTreeMap;

pub(crate) const DEFAULT_TOUCH_INTERVAL_MS: u64 = 100;

#[derive(Debug, Clone, Default)]
pub(crate) struct TouchBatch {
    pending: BTreeMap<i32, Touch>,
    interval_ms: Option<u64>,
    last_flush_ms: Option<u64>,
    unreported: bool,
    enabled: bool,
    target: String,
}

impl TouchBatch {
    fn interval(&self) -> u64 {
        self.interval_ms.unwrap_or(DEFAULT_TOUCH_INTERVAL_MS)
    }

    /// A finger that has not moved is not news, and one that has only just
    /// arrived stays new until the set it arrived in has gone.
    fn move_to(touch: &mut Touch, x: f64, y: f64) {
        if touch.x == x && touch.y == y {
            return;
        }
        touch.x = x;
        touch.y = y;
        if touch.state == TouchState::Stationary {
            touch.state = TouchState::Moved;
        }
    }

    fn apply(&mut self, event: TouchEvent) {
        self.unreported = true;
        match event {
            TouchEvent::CancelAll => {
                for touch in self.pending.values_mut() {
                    touch.state = TouchState::Cancelled;
                }
            }
            TouchEvent::Pointer {
                id,
                x,
                y,
                phase,
                screen_width,
                screen_height,
            } => match phase {
                TouchPhase::Began => {
                    self.pending.insert(
                        id,
                        Touch {
                            id,
                            x,
                            y,
                            screen_width,
                            screen_height,
                            state: TouchState::Began,
                        },
                    );
                }
                // A finger reports where it last was, not where the release
                // landed.
                TouchPhase::Ended => {
                    if let Some(touch) = self.pending.get_mut(&id) {
                        touch.state = TouchState::Ended;
                    }
                }
                TouchPhase::Cancelled => {
                    if let Some(touch) = self.pending.get_mut(&id) {
                        touch.state = TouchState::Cancelled;
                    }
                }
                TouchPhase::Moved => {
                    if let Some(touch) = self.pending.get_mut(&id) {
                        Self::move_to(touch, x, y);
                    }
                }
            },
        }
    }

    fn advance(&mut self) {
        self.pending
            .retain(|_, touch| !matches!(touch.state, TouchState::Ended | TouchState::Cancelled));
        for touch in self.pending.values_mut() {
            if matches!(touch.state, TouchState::Began | TouchState::Moved) {
                touch.state = TouchState::Stationary;
            }
        }
    }
}

impl Engine {
    pub(crate) fn set_touch_interval(&mut self, ms: i32) {
        self.touch.interval_ms = (ms > 0).then_some(ms as u64);
    }

    /// What a control scheme said about touch. A scheme declares that its
    /// layout needs raw touch; it does not forbid it. So a scheme silent on
    /// touch leaves alone whatever a game turned on for its own hit testing,
    /// and only the game can take it away again.
    pub(crate) fn declare_touch(&mut self, enabled: bool) {
        if enabled {
            self.set_touch_enabled(true);
        }
    }

    /// Switching touch on starts an empty set and a fresh window, so a session
    /// never inherits fingers from the one before it.
    pub(crate) fn set_touch_enabled(&mut self, enabled: bool) {
        if self.touch.enabled == enabled {
            return;
        }
        self.touch.enabled = enabled;
        self.touch.pending.clear();
        self.touch.last_flush_ms = self.clock_ms;
    }

    pub(crate) fn take_touch_events(
        &mut self,
        target: &str,
        events: Vec<TouchEvent>,
        next_send_ms: &mut Option<u64>,
    ) -> Vec<Outgoing> {
        if !self.touch.enabled {
            return Vec::new();
        }
        if !target.is_empty() {
            self.touch.target = target.to_string();
        }
        for event in events {
            self.touch.apply(event);
        }

        let Some(now) = self.clock_ms else {
            return self.flush_touches();
        };

        // Fresh input goes at half the interval. A set with nothing new in it
        // repeats at the whole one, which run_due fires instead.
        let outgoings = match self.touch.last_flush_ms {
            Some(last) if now < last + self.touch.interval() / 2 => Vec::new(),
            _ => self.flush_touches(),
        };
        *next_send_ms = self
            .touch
            .last_flush_ms
            .map(|last| last + self.touch.interval() / 2);
        outgoings
    }

    fn flush_touches(&mut self) -> Vec<Outgoing> {
        if self.touch.pending.is_empty() {
            return Vec::new();
        }
        let touches: Vec<Touch> = self.touch.pending.values().cloned().collect();
        let target = self.touch.target.clone();
        let reliability = self.reliability_for(&target, ChannelType::Touch.value());

        self.touch.last_flush_ms = self.clock_ms;
        self.touch.unreported = false;
        self.touch.advance();
        self.make_touch_set(&target, touches, reliability)
    }

    /// Input held back for the cadence still has to go, and no further event
    /// is coming once a finger stops moving. Fresh input goes at half the
    /// interval, where a set with nothing new in it repeats at the whole one.
    pub(crate) fn touch_flush_due(&self) -> Option<u64> {
        if !self.touch.unreported || self.touch.pending.is_empty() {
            return None;
        }
        self.touch
            .last_flush_ms
            .map(|last| last + self.touch.interval() / 2)
    }

    /// A set that has already gone still repeats while a finger is down, so a
    /// game that lost the datagram is not left holding a stale position for as
    /// long as the finger lasts.
    pub(crate) fn touch_repeat_due(&self) -> Option<u64> {
        let unreliable = self.reliability_for(&self.touch.target, ChannelType::Touch.value())
            == BMReliability::Unreliable.code();
        if self.touch.pending.is_empty() || !unreliable {
            return None;
        }
        self.touch
            .last_flush_ms
            .map(|last| last + self.touch.interval())
    }

    pub(crate) fn run_touch_due(&mut self, now: u64, out: &mut Vec<Outgoing>) {
        let due = |at: Option<u64>| at.is_some_and(|at| at <= now);
        if due(self.touch_flush_due()) || due(self.touch_repeat_due()) {
            out.extend(self.flush_touches());
        }
    }

    pub(crate) fn reset_touch(&mut self) {
        self.touch = TouchBatch::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::object::Object;
    use crate::config::EngineConfig;
    use crate::devices::device_core::DeviceCore;
    use crate::engine::device_registry::DeviceRecord;
    use crate::engine::events::{Command, ProcessOutput};
    use crate::policy::EndpointMode;
    use crate::types::device_type::DeviceType;

    fn controller() -> Engine {
        let mut eng = Engine::default();
        eng.init_local_device(DeviceCore::new(
            "phone".to_string(),
            "Phone".to_string(),
            DeviceType::Android,
        ));
        eng.configure(EngineConfig {
            endpoint: Some(EndpointMode::Controller),
            opens_sessions: false,
            ..Default::default()
        })
        .unwrap();
        eng.push_registry_update(DeviceRecord::new(
            DeviceCore::new("game".to_string(), "Game".to_string(), DeviceType::Unity),
            None,
        ));
        eng.declare_touch(true);
        eng
    }

    fn pointer(id: i32, x: f64, phase: TouchPhase) -> TouchEvent {
        TouchEvent::Pointer {
            id,
            x,
            y: 10.0,
            phase,
            screen_width: 480,
            screen_height: 320,
        }
    }

    fn feed(eng: &mut Engine, now_ms: u64, events: Vec<TouchEvent>) -> ProcessOutput {
        eng.emit(
            Command::TouchEvent {
                target: "game".to_string(),
                events,
            },
            Some(now_ms),
        )
        .unwrap()
    }

    fn sent(out: &ProcessOutput) -> Vec<Touch> {
        let mut pkt = crate::codec::externals::bm_packet::BMPacket::default();
        crate::engine::protocol::deserialize_message(out.outgoings[0].message(), &mut pkt).unwrap();
        let msg = pkt.message.expect("a touch packet carries a message");
        let mut cur = crate::codec::bm_stream::BMStream::view(msg.as_slice());
        match Object::decode(&mut cur) {
            Ok(Object::TouchSet(set)) => set.touches,
            other => panic!("expected a touch set, got {other:?}"),
        }
    }

    /// A finger arrives as new and stays new until the set carrying it has
    /// gone, however far it moves in between.
    #[test]
    fn a_finger_stays_new_until_it_has_been_reported() {
        let mut eng = controller();
        let out = feed(
            &mut eng,
            0,
            vec![
                pointer(0, 1.0, TouchPhase::Began),
                pointer(0, 5.0, TouchPhase::Moved),
            ],
        );
        let touches = sent(&out);
        assert_eq!(touches.len(), 1);
        assert_eq!(touches[0].state, TouchState::Began, "still new");
        assert_eq!(touches[0].x, 5.0, "but where it got to");
    }

    /// Only a finger that had settled can move again.
    #[test]
    fn a_settled_finger_moves_and_a_still_one_says_nothing() {
        let mut eng = controller();
        feed(&mut eng, 0, vec![pointer(0, 1.0, TouchPhase::Began)]);

        let same = feed(&mut eng, 100, vec![pointer(0, 1.0, TouchPhase::Moved)]);
        assert_eq!(
            sent(&same)[0].state,
            TouchState::Stationary,
            "it did not actually move"
        );

        let moved = feed(&mut eng, 200, vec![pointer(0, 9.0, TouchPhase::Moved)]);
        assert_eq!(sent(&moved)[0].state, TouchState::Moved);
        assert_eq!(sent(&moved)[0].x, 9.0);
    }

    /// A lifted finger reports where it last was, and goes exactly once.
    #[test]
    fn a_lifted_finger_is_reported_once_and_keeps_its_place() {
        let mut eng = controller();
        feed(&mut eng, 0, vec![pointer(0, 3.0, TouchPhase::Began)]);

        let up = feed(&mut eng, 100, vec![pointer(0, 99.0, TouchPhase::Ended)]);
        let touches = sent(&up);
        assert_eq!(touches[0].state, TouchState::Ended);
        assert_eq!(touches[0].x, 3.0, "not where the release landed");

        assert!(
            feed(&mut eng, 200, vec![]).outgoings.is_empty(),
            "and then it is gone"
        );
    }

    /// Some platforms take a whole gesture away at once, others one finger.
    #[test]
    fn a_gesture_can_be_taken_away_whole_or_a_finger_at_a_time() {
        let mut eng = controller();
        feed(
            &mut eng,
            0,
            vec![
                pointer(0, 1.0, TouchPhase::Began),
                pointer(1, 2.0, TouchPhase::Began),
            ],
        );
        let whole = feed(&mut eng, 100, vec![TouchEvent::CancelAll]);
        let touches = sent(&whole);
        assert_eq!(touches.len(), 2);
        assert!(touches.iter().all(|t| t.state == TouchState::Cancelled));

        let mut eng = controller();
        feed(
            &mut eng,
            0,
            vec![
                pointer(0, 1.0, TouchPhase::Began),
                pointer(1, 2.0, TouchPhase::Began),
            ],
        );
        let one = feed(&mut eng, 100, vec![pointer(0, 1.0, TouchPhase::Cancelled)]);
        let touches = sent(&one);
        assert_eq!(touches[0].state, TouchState::Cancelled);
        assert_eq!(touches[1].state, TouchState::Stationary, "the other stays");
    }

    #[test]
    fn input_goes_out_at_half_the_interval_the_game_asked_for() {
        let mut eng = controller();
        feed(&mut eng, 0, vec![pointer(0, 1.0, TouchPhase::Began)]);

        let early = feed(&mut eng, 49, vec![pointer(0, 2.0, TouchPhase::Moved)]);
        assert!(early.outgoings.is_empty(), "too soon");
        assert_eq!(early.next_send_ms, Some(50));

        let due = feed(&mut eng, 50, vec![pointer(0, 3.0, TouchPhase::Moved)]);
        assert_eq!(due.outgoings.len(), 1);
        assert_eq!(sent(&due)[0].x, 3.0, "carrying everything held back");
    }

    #[test]
    fn nothing_is_sent_when_no_finger_is_down() {
        let mut eng = controller();
        assert!(feed(&mut eng, 0, vec![]).outgoings.is_empty());
        assert!(feed(&mut eng, 1000, vec![]).outgoings.is_empty());
    }

    /// A held finger repeats while it is down, so a lost datagram does not
    /// strand the game on a stale position.
    #[test]
    fn a_held_finger_keeps_being_reported() {
        let mut eng = controller();
        feed(&mut eng, 0, vec![pointer(0, 1.0, TouchPhase::Began)]);

        assert!(eng.handle_time(99).outgoings.is_empty(), "not yet");

        let repeat = eng.handle_time(100);
        assert_eq!(repeat.outgoings.len(), 1);
        assert_eq!(sent(&repeat)[0].state, TouchState::Stationary);

        assert_eq!(
            eng.handle_time(200).outgoings.len(),
            1,
            "and it does not stop at three"
        );
        assert_eq!(eng.handle_time(600).outgoings.len(), 1);
    }

    /// Input arriving exactly when a repeat is owed goes out as itself, rather
    /// than losing its turn to a repeat of the set it supersedes.
    #[test]
    fn input_on_a_repeat_boundary_is_not_beaten_by_the_repeat() {
        let mut eng = controller();
        feed(&mut eng, 0, vec![pointer(0, 1.0, TouchPhase::Began)]);

        let due = feed(&mut eng, 100, vec![pointer(0, 7.0, TouchPhase::Moved)]);
        assert_eq!(due.outgoings.len(), 1, "one packet, not two");
        let touches = sent(&due);
        assert_eq!(touches[0].state, TouchState::Moved);
        assert_eq!(touches[0].x, 7.0);
    }

    /// The end of a swipe is the position that matters, and no further event
    /// is coming once the finger stops. It must not wait for the next tap.
    #[test]
    fn the_last_move_of_a_swipe_arrives_without_another_touch() {
        let mut eng = controller();
        feed(&mut eng, 0, vec![pointer(0, 1.0, TouchPhase::Began)]);

        let held = feed(&mut eng, 10, vec![pointer(0, 9.0, TouchPhase::Moved)]);
        assert!(held.outgoings.is_empty(), "too soon for the wire");
        assert_eq!(held.next_time_ms, Some(50), "but the clock is told");

        let flushed = eng.handle_time(50);
        assert_eq!(flushed.outgoings.len(), 1, "and it goes on its own");
        assert_eq!(sent(&flushed)[0].x, 9.0);
    }

    /// The same holds when the game asked for a stream, where there is no
    /// repeat to fall back on.
    #[test]
    fn held_back_input_still_arrives_when_touch_is_reliable() {
        let mut eng = controller();
        eng.set_input_reliability(Some(BMReliability::ReliableOrdered.code()), None);
        feed(&mut eng, 0, vec![pointer(0, 1.0, TouchPhase::Began)]);

        let held = feed(&mut eng, 10, vec![pointer(0, 4.0, TouchPhase::Moved)]);
        assert!(held.outgoings.is_empty());
        assert_eq!(
            held.next_time_ms,
            Some(50),
            "a stream has no repeat, so this is the only thing that will send it"
        );
        assert_eq!(sent(&eng.handle_time(50))[0].x, 4.0);
    }

    /// Nothing new means nothing owed, so a settled set does not keep waking
    /// the caller for a flush it does not need.
    #[test]
    fn a_set_with_nothing_new_owes_no_flush() {
        let mut eng = controller();
        eng.set_input_reliability(Some(BMReliability::ReliableOrdered.code()), None);
        feed(&mut eng, 0, vec![pointer(0, 1.0, TouchPhase::Began)]);

        assert_eq!(eng.touch_flush_due(), None, "the set has just gone");
        assert_eq!(eng.handle_time(500).next_time_ms, None);
    }

    /// A stream either delivers or breaks, so there is nothing to repeat.
    #[test]
    fn a_reliable_set_is_not_repeated() {
        let mut eng = controller();
        eng.set_input_reliability(Some(BMReliability::ReliableOrdered.code()), None);
        feed(&mut eng, 0, vec![pointer(0, 1.0, TouchPhase::Began)]);

        assert!(eng.handle_time(100).outgoings.is_empty());
        assert_eq!(eng.touch_repeat_due(), None);
    }

    #[test]
    fn the_game_sets_the_rate() {
        let mut eng = controller();
        eng.set_touch_interval(40);
        feed(&mut eng, 0, vec![pointer(0, 1.0, TouchPhase::Began)]);

        assert!(
            feed(&mut eng, 19, vec![pointer(0, 2.0, TouchPhase::Moved)])
                .outgoings
                .is_empty()
        );
        assert_eq!(
            feed(&mut eng, 20, vec![pointer(0, 3.0, TouchPhase::Moved)])
                .outgoings
                .len(),
            1
        );
    }

    /// Switching touch off and on again is a clean slate.
    #[test]
    fn switching_touch_on_forgets_the_fingers_before_it() {
        let mut eng = controller();
        feed(&mut eng, 0, vec![pointer(0, 1.0, TouchPhase::Began)]);
        eng.set_touch_enabled(false);
        eng.set_touch_enabled(true);

        assert!(feed(&mut eng, 500, vec![]).outgoings.is_empty());
    }

    /// A game that turns touch off is not sent touch, and nothing is left
    /// repeating behind it.
    #[test]
    fn a_game_that_turns_touch_off_stops_receiving_it() {
        let mut eng = controller();
        feed(&mut eng, 0, vec![pointer(0, 1.0, TouchPhase::Began)]);
        eng.set_touch_enabled(false);

        let after = feed(&mut eng, 100, vec![pointer(0, 2.0, TouchPhase::Moved)]);
        assert!(after.outgoings.is_empty());
        assert_eq!(eng.touch_repeat_due(), None);
        assert!(eng.handle_time(1000).outgoings.is_empty());
    }

    /// Nothing has said touch is wanted, so nothing is sent.
    #[test]
    fn touch_is_not_sent_until_something_asks_for_it() {
        let mut eng = Engine::default();
        eng.init_local_device(DeviceCore::new(
            "phone".to_string(),
            "Phone".to_string(),
            DeviceType::Android,
        ));
        eng.push_registry_update(DeviceRecord::new(
            DeviceCore::new("game".to_string(), "Game".to_string(), DeviceType::Unity),
            None,
        ));

        assert!(
            feed(&mut eng, 0, vec![pointer(0, 1.0, TouchPhase::Began)])
                .outgoings
                .is_empty()
        );

        eng.declare_touch(true);
        assert_eq!(
            feed(&mut eng, 100, vec![pointer(0, 1.0, TouchPhase::Began)])
                .outgoings
                .len(),
            1
        );
    }

    /// A scheme says its layout needs touch; it does not say a game may not
    /// have it. So one that is silent leaves a game's own choice standing, and
    /// only the game can take it back.
    #[test]
    fn a_scheme_can_ask_for_touch_but_cannot_refuse_it() {
        let mut eng = controller();
        eng.declare_touch(false);
        assert_eq!(
            feed(&mut eng, 0, vec![pointer(0, 1.0, TouchPhase::Began)])
                .outgoings
                .len(),
            1,
            "the silent scheme changed nothing"
        );

        eng.set_touch_enabled(false);
        eng.declare_touch(false);
        assert!(
            feed(&mut eng, 100, vec![pointer(1, 1.0, TouchPhase::Began)])
                .outgoings
                .is_empty(),
            "and cannot undo the game's refusal either"
        );

        eng.declare_touch(true);
        assert_eq!(
            feed(&mut eng, 200, vec![pointer(2, 1.0, TouchPhase::Began)])
                .outgoings
                .len(),
            1,
            "but a scheme that asks is answered"
        );
    }

    /// A caller that keeps no clock keeps the behaviour it had.
    #[test]
    fn without_a_clock_every_batch_goes() {
        let mut eng = controller();
        for x in 0..4 {
            let out = eng
                .emit(
                    Command::TouchEvent {
                        target: "game".to_string(),
                        events: vec![pointer(0, x as f64, TouchPhase::Began)],
                    },
                    None,
                )
                .unwrap();
            assert_eq!(out.outgoings.len(), 1);
        }
    }

    /// The C binding carries commands as named msgpack, and nothing on either
    /// side of it is checked by a compiler.
    #[test]
    fn the_keys_a_caller_writes_are_the_keys_that_are_read() {
        #[derive(serde::Serialize)]
        struct Pointer {
            #[serde(rename = "type")]
            kind: &'static str,
            id: i32,
            x: f64,
            y: f64,
            phase: &'static str,
            screen_width: i16,
            screen_height: i16,
        }
        #[derive(serde::Serialize)]
        struct Batch {
            #[serde(rename = "type")]
            kind: &'static str,
            target: &'static str,
            events: Vec<Pointer>,
        }

        let bytes = rmp_serde::to_vec_named(&Batch {
            kind: "TouchEvent",
            target: "game",
            events: vec![Pointer {
                kind: "Pointer",
                id: 2,
                x: 1.5,
                y: 2.5,
                phase: "Began",
                screen_width: 480,
                screen_height: 320,
            }],
        })
        .unwrap();

        let cmd: Command = rmp_serde::from_slice(&bytes).expect("reads as a command");
        match cmd {
            Command::TouchEvent { target, events } => {
                assert_eq!(target, "game");
                assert_eq!(
                    events,
                    vec![TouchEvent::Pointer {
                        id: 2,
                        x: 1.5,
                        y: 2.5,
                        phase: TouchPhase::Began,
                        screen_width: 480,
                        screen_height: 320,
                    }]
                );
            }
            other => panic!("expected a touch batch, got {other:?}"),
        }
    }

    /// A gesture taken away whole carries nothing but its name.
    #[test]
    fn taking_a_gesture_away_needs_no_fields() {
        #[derive(serde::Serialize)]
        struct Tagged {
            #[serde(rename = "type")]
            kind: &'static str,
        }
        let bytes = rmp_serde::to_vec_named(&Tagged { kind: "CancelAll" }).unwrap();
        let event: TouchEvent = rmp_serde::from_slice(&bytes).expect("reads as an event");
        assert_eq!(event, TouchEvent::CancelAll);
    }

    /// The escape hatch: a set the caller built itself goes as it is, around
    /// all of this.
    #[test]
    fn a_caller_can_still_send_a_set_it_built_itself() {
        let mut eng = controller();
        let out = eng
            .emit(
                Command::SendTouch {
                    target: "game".to_string(),
                    touches: vec![Touch {
                        id: 7,
                        x: 4.0,
                        y: 5.0,
                        screen_width: 480,
                        screen_height: 320,
                        state: TouchState::Moved,
                    }],
                },
                Some(0),
            )
            .unwrap();
        assert_eq!(out.outgoings.len(), 1);
        assert_eq!(sent(&out)[0].id, 7);
        assert_eq!(eng.touch_repeat_due(), None, "and the engine holds nothing");
    }
}
