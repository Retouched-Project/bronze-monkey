// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

pub mod actions;
pub mod events;
pub mod ffi;
pub mod processing;
pub mod protocol;
#[cfg(feature = "pyo3")]
pub mod python;
pub mod registry;
pub mod state;
#[cfg(target_arch = "wasm32")]
pub mod wasm;

pub use actions::Action;
pub use events::{Command, ControlConfig, Event, Outgoing, ProcessOutput};
pub use processing::Engine;
pub use registry::{DeviceRecord, DeviceRegistry};
pub use state::EngineState;
