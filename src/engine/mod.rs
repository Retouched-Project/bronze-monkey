// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

pub mod cffi;
pub mod device_registry;
pub mod events;
pub mod methods;
pub mod processing;
pub mod protocol;
#[cfg(feature = "pyo3")]
pub mod python;
pub mod state;
#[cfg(target_arch = "wasm32")]
pub mod wasm;

pub use device_registry::{DeviceRecord, DeviceRegistry};
pub use events::{
    Arrival, Command, ControlConfig, EmitError, Event, Outgoing, ProcessOutput, Sensor, Via,
};
pub use processing::Engine;
pub use state::EngineState;
