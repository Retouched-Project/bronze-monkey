// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey
//
//! The engine's side of the clock.
//!
//! The caller owns the clock and the timers; the engine owns the deadlines.
//! Time reaches the engine as a fact, either riding an arrival or through
//! [`Engine::handle_time`], and every answer names the next moment the engine
//! wants to hear from the clock. A caller that never supplies time gets no
//! deadlines and no scheduled behaviour, and everything else works unchanged.

use super::Engine;
use crate::engine::events::ProcessOutput;

pub(crate) const PING_INTERVAL_MS: u64 = 60_000;

impl Engine {
    /// Tells the engine what time it is, with no bytes attached.
    ///
    /// Anything due fires, and the answer names the next wanted moment.
    /// Calling early is harmless; the same deadline comes back.
    pub fn handle_time(&mut self, now_ms: u64) -> ProcessOutput {
        let mut out = ProcessOutput::new();
        self.advance_clock(now_ms, &mut out);
        out.next_time_ms = self.next_deadline();
        out
    }

    /// Moves the clock forward and fires whatever came due. The clock never
    /// runs backwards: an older reading than the one already seen is treated
    /// as now.
    pub(crate) fn advance_clock(&mut self, now_ms: u64, out: &mut ProcessOutput) {
        let now = self.set_clock(now_ms);
        self.run_due(now, out);
    }

    pub(crate) fn set_clock(&mut self, now_ms: u64) -> u64 {
        let now = self.clock_ms.map_or(now_ms, |seen| seen.max(now_ms));
        self.clock_ms = Some(now);
        now
    }

    pub(crate) fn run_due(&mut self, now: u64, out: &mut ProcessOutput) {
        self.run_touch_due(now, &mut out.outgoings);
        if self.roles.game() {
            self.schedule_pings(now);
            let due: Vec<String> = self
                .ping_at
                .iter()
                .filter(|(_, at)| **at <= now)
                .map(|(id, _)| id.clone())
                .collect();
            for id in due {
                out.outgoings.extend(self.make_ping_packet(&id));
                self.ping_at.insert(id, now + PING_INTERVAL_MS);
            }
        }
    }

    /// A game owes each acked controller a ping. A peer acked before the
    /// clock existed is picked up the first time it does.
    fn schedule_pings(&mut self, now: u64) {
        let acked = &self.state.acked_peers;
        let ping_at = &mut self.ping_at;
        for id in acked {
            ping_at.entry(id.clone()).or_insert(now + PING_INTERVAL_MS);
        }
    }

    pub(crate) fn next_deadline(&self) -> Option<u64> {
        [
            self.ping_at.values().min().copied(),
            self.touch_flush_due(),
            self.touch_repeat_due(),
        ]
        .into_iter()
        .flatten()
        .min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EngineConfig;
    use crate::devices::device_core::DeviceCore;
    use crate::engine::device_registry::DeviceRecord;
    use crate::engine::events::Arrival;
    use crate::engine::protocol::deserialize_message;
    use crate::policy::EndpointMode;
    use crate::types::device_type::DeviceType;
    use crate::types::packet_type::PacketType;

    fn game() -> Engine {
        let mut eng = Engine::default();
        eng.init_local_device(DeviceCore::new(
            "game".to_string(),
            "Game".to_string(),
            DeviceType::Unity,
        ));
        eng.configure(EngineConfig {
            endpoint: Some(EndpointMode::Game),
            opens_sessions: false,
            ..Default::default()
        })
        .unwrap();
        eng.push_registry_update(DeviceRecord::new(
            DeviceCore::new(
                "phone".to_string(),
                "Phone".to_string(),
                DeviceType::Android,
            ),
            None,
        ));
        eng
    }

    fn ping_from(controller: &str) -> Vec<u8> {
        let mut eng = Engine::default();
        eng.init_local_device(DeviceCore::new(
            controller.to_string(),
            "Phone".to_string(),
            DeviceType::Android,
        ));
        eng.push_registry_update(DeviceRecord::new(
            DeviceCore::new("game".to_string(), "Game".to_string(), DeviceType::Unity),
            None,
        ));
        eng.make_ping_packet("game").remove(0).message().to_vec()
    }

    fn at(now_ms: u64) -> Arrival {
        Arrival {
            now_ms: Some(now_ms),
            ..Default::default()
        }
    }

    #[test]
    fn a_game_probes_a_connected_controller_once_a_minute() {
        let mut eng = game();
        let out = eng.process_incoming(&ping_from("phone"), &at(0));
        assert_eq!(out.outgoings.len(), 1, "the first ping is acked");
        assert_eq!(out.next_time_ms, Some(60_000), "and a probe is owed");

        let quiet = eng.handle_time(59_999);
        assert!(quiet.outgoings.is_empty(), "nothing is due early");
        assert_eq!(quiet.next_time_ms, Some(60_000));

        let due = eng.handle_time(60_000);
        assert_eq!(due.outgoings.len(), 1);
        assert_eq!(due.outgoings[0].target_device_id, "phone");
        let mut pkt = crate::codec::externals::bm_packet::BMPacket::default();
        deserialize_message(due.outgoings[0].message(), &mut pkt).unwrap();
        assert_eq!(pkt.packet_type, PacketType::Ping);
        assert_eq!(due.next_time_ms, Some(120_000));
    }

    #[test]
    fn a_late_clock_owes_one_ping_not_a_burst() {
        let mut eng = game();
        eng.process_incoming(&ping_from("phone"), &at(0));

        let due = eng.handle_time(330_000);
        assert_eq!(due.outgoings.len(), 1, "five missed intervals, one ping");
        assert_eq!(
            due.next_time_ms,
            Some(390_000),
            "and the cadence restarts from now"
        );
    }

    #[test]
    fn no_clock_means_nothing_scheduled() {
        let mut eng = game();
        let out = eng.process_incoming(&ping_from("phone"), &Arrival::default());
        assert_eq!(out.outgoings.len(), 1, "the ack does not need a clock");
        assert_eq!(
            out.next_time_ms, None,
            "but nothing is scheduled without one"
        );
    }

    #[test]
    fn a_peer_acked_before_the_clock_is_picked_up_by_it() {
        let mut eng = game();
        eng.process_incoming(&ping_from("phone"), &Arrival::default());

        let first = eng.handle_time(7000);
        assert!(first.outgoings.is_empty(), "the first tick only schedules");
        assert_eq!(first.next_time_ms, Some(67_000));
        assert_eq!(eng.handle_time(67_000).outgoings.len(), 1);
    }

    #[test]
    fn a_departed_peer_takes_its_ping_along() {
        let mut eng = game();
        eng.process_incoming(&ping_from("phone"), &at(0));
        eng.peer_gone("phone");

        let out = eng.handle_time(120_000);
        assert!(out.outgoings.is_empty());
        assert_eq!(out.next_time_ms, None);
    }
}
