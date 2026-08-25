// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey
//
//! When a sensor reading is allowed onto the wire.
//!
//! A game says how often it wants a sensor, and a controller that sends faster
//! floods it: the input queue backs up and motion keeps playing out after the
//! hand has stopped. So a reading that arrives before its interval is over is
//! dropped rather than held. Nothing is buffered, and the next reading to
//! arrive after the boundary is the one that goes.
//!
//! Reading a sensor is the caller's business. Whether a reading it has already
//! taken belongs on the wire is this module's.

use super::Engine;
use crate::engine::events::Sensor;

pub(crate) const DEFAULT_SENSOR_INTERVAL_MS: u64 = 100;

#[derive(Debug, Clone, Default)]
pub(crate) struct Gate {
    interval_ms: Option<u64>,
    last_dispatch_ms: Option<u64>,
    enabled: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SensorPacing {
    accel: Gate,
    gyro: Gate,
    orientation: Gate,
}

impl SensorPacing {
    fn gate_mut(&mut self, sensor: Sensor) -> Option<&mut Gate> {
        match sensor {
            Sensor::Accel => Some(&mut self.accel),
            Sensor::Gyro => Some(&mut self.gyro),
            Sensor::Orientation => Some(&mut self.orientation),
            Sensor::Touch => None,
        }
    }
}

impl Engine {
    pub(crate) fn set_sensor_interval(&mut self, sensor: Sensor, ms: i32) {
        if let Some(gate) = self.sensor_pacing.gate_mut(sensor) {
            gate.interval_ms = (ms > 0).then_some(ms as u64);
        }
    }

    /// A sensor turned on waits a whole interval before its first send, so
    /// switching one on does not put a reading on the wire immediately.
    /// Turning one off that is already off changes nothing.
    pub(crate) fn set_sensor_enabled(&mut self, sensor: Sensor, enabled: bool) {
        if let Some(gate) = self.sensor_pacing.gate_mut(sensor)
            && gate.enabled != enabled
        {
            gate.enabled = enabled;
            gate.last_dispatch_ms = None;
        }
    }

    /// Takes what a game said about its sensors out of a config on its way
    /// past. The cadence is the engine's to keep; the consumer is told the
    /// same thing so it can decide what to sample.
    pub(crate) fn note_sensor_config(&mut self, cfg: &crate::engine::events::ControlConfig) {
        let each = [
            (Sensor::Accel, cfg.accel_enabled, cfg.accel_interval_ms),
            (Sensor::Gyro, cfg.gyro_enabled, cfg.gyro_interval_ms),
            (
                Sensor::Orientation,
                cfg.orientation_enabled,
                cfg.orientation_interval_ms,
            ),
        ];
        for (sensor, enabled, interval_ms) in each {
            if let Some(ms) = interval_ms {
                self.set_sensor_interval(sensor, ms);
            }
            if let Some(enabled) = enabled {
                self.set_sensor_enabled(sensor, enabled);
            }
        }
    }

    /// Whether a reading may go out, moving the boundary on when it may.
    ///
    /// Without a clock there is no boundary to be on the wrong side of, so
    /// every reading passes.
    pub(crate) fn sensor_due(&mut self, sensor: Sensor) -> bool {
        let Some(now) = self.clock_ms else {
            return true;
        };
        let Some(gate) = self.sensor_pacing.gate_mut(sensor) else {
            return true;
        };

        let interval = gate.interval_ms.unwrap_or(DEFAULT_SENSOR_INTERVAL_MS);
        if interval == 0 {
            return true;
        }

        let Some(last) = gate.last_dispatch_ms else {
            // The first reading seen starts the interval rather than riding it.
            gate.last_dispatch_ms = Some(now);
            return false;
        };

        let next_at = last + interval;
        if now < next_at {
            return false;
        }
        // Land on the last boundary at or before now. Advancing to now instead
        // would let a late reading carry the whole cadence later with it.
        gate.last_dispatch_ms = Some(((now - next_at) / interval) * interval + next_at);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EngineConfig;
    use crate::devices::device_core::DeviceCore;
    use crate::engine::device_registry::DeviceRecord;
    use crate::engine::events::Command;
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
        eng
    }

    fn accel_at(eng: &mut Engine, now_ms: u64) -> usize {
        eng.emit(
            Command::SendAccel {
                target: "game".to_string(),
                x: 0.1,
                y: 0.2,
                z: 0.3,
            },
            Some(now_ms),
        )
        .unwrap()
        .outgoings
        .len()
    }

    /// A game that named nothing still gets a cadence, and it is the one both
    /// sides of the protocol default to.
    #[test]
    fn a_sensor_a_game_said_nothing_about_goes_ten_times_a_second() {
        let mut eng = controller();

        assert_eq!(
            accel_at(&mut eng, 0),
            0,
            "the first reading starts the wait"
        );
        assert_eq!(accel_at(&mut eng, 40), 0);
        assert_eq!(accel_at(&mut eng, 99), 0);
        assert_eq!(accel_at(&mut eng, 100), 1, "and the boundary lets one out");
        assert_eq!(accel_at(&mut eng, 140), 0, "the next wait starts at once");
        assert_eq!(accel_at(&mut eng, 200), 1);
    }

    /// A game that asks for a cadence gets it, which is the whole point of
    /// holding the interval rather than handing it away.
    #[test]
    fn a_game_that_names_an_interval_is_obeyed() {
        let mut eng = controller();
        eng.set_sensor_interval(Sensor::Accel, 33);

        assert_eq!(accel_at(&mut eng, 0), 0);
        assert_eq!(accel_at(&mut eng, 32), 0);
        assert_eq!(accel_at(&mut eng, 33), 1);
        assert_eq!(accel_at(&mut eng, 66), 1);
    }

    /// A reading arriving late lands on the boundary it passed, not on itself,
    /// so a stalled sensor does not drag the cadence along behind it.
    #[test]
    fn a_late_reading_does_not_drag_the_cadence_with_it() {
        let mut eng = controller();
        accel_at(&mut eng, 0);

        assert_eq!(accel_at(&mut eng, 450), 1, "very late, and still just one");
        // Had the boundary moved to 450, the next would not be owed until 550.
        assert_eq!(accel_at(&mut eng, 500), 1, "the grid was kept");
    }

    /// Each sensor is paced on its own, so a fast one cannot spend another's turn.
    #[test]
    fn one_sensor_does_not_spend_another_sensors_turn() {
        let mut eng = controller();
        eng.set_sensor_interval(Sensor::Gyro, 1000);
        accel_at(&mut eng, 0);

        let gyro = eng
            .emit(
                Command::SendGyro {
                    target: "game".to_string(),
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                Some(100),
            )
            .unwrap();
        assert!(gyro.outgoings.is_empty(), "the gyro is on its own clock");
        assert_eq!(accel_at(&mut eng, 100), 1, "and the accel keeps its own");
    }

    /// Switching a sensor on starts its interval, so enabling one mid-session
    /// does not put a reading on the wire the same instant.
    #[test]
    fn switching_a_sensor_on_starts_its_interval() {
        let mut eng = controller();
        accel_at(&mut eng, 0);
        assert_eq!(accel_at(&mut eng, 100), 1);

        eng.set_sensor_enabled(Sensor::Accel, true);
        assert_eq!(accel_at(&mut eng, 100), 0, "the wait begins again");
        assert_eq!(accel_at(&mut eng, 199), 0);
        assert_eq!(accel_at(&mut eng, 200), 1);
    }

    /// A game asking for nothing sensible gets the default rather than the
    /// firehose. An unpaced sensor is the failure this gate exists to prevent,
    /// so nonsense must not be the one way to switch it off.
    #[test]
    fn an_interval_of_nothing_falls_back_rather_than_opening_the_tap() {
        for asked in [0, -1, i32::MIN] {
            let mut eng = controller();
            eng.set_sensor_interval(Sensor::Accel, asked);

            assert_eq!(accel_at(&mut eng, 0), 0, "asked for {asked}");
            assert_eq!(accel_at(&mut eng, 99), 0, "asked for {asked}");
            assert_eq!(accel_at(&mut eng, 100), 1, "asked for {asked}");
        }
    }

    /// A caller that keeps no clock keeps the behaviour it had.
    #[test]
    fn without_a_clock_every_reading_goes() {
        let mut eng = controller();
        for _ in 0..5 {
            let out = eng
                .emit(
                    Command::SendAccel {
                        target: "game".to_string(),
                        x: 0.1,
                        y: 0.2,
                        z: 0.3,
                    },
                    None,
                )
                .unwrap();
            assert_eq!(out.outgoings.len(), 1);
        }
    }

    /// The next game may want a different cadence, so nothing outlives the session.
    #[test]
    fn a_new_session_inherits_no_cadence() {
        let mut eng = controller();
        eng.set_sensor_interval(Sensor::Accel, 1000);
        accel_at(&mut eng, 0);
        assert_eq!(accel_at(&mut eng, 100), 0, "the long interval holds");

        eng.reset_game_session();
        accel_at(&mut eng, 100);
        assert_eq!(accel_at(&mut eng, 200), 1, "and the default is back");
    }
}
