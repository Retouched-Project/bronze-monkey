// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use serde::{Deserialize, Serialize};

pub mod controller;
pub mod game;
pub mod server;

pub use controller::{ControllerPolicy, InputReliability, SessionInputs, Viewport};
pub use game::GamePolicy;
pub use server::ServerPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "i32", try_from = "i32")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
#[cfg_attr(feature = "pyo3", pyo3::pyclass(eq, eq_int, from_py_object))]
#[repr(i32)]
pub enum EndpointMode {
    Game = 1,
    Controller = 2,
}

impl EndpointMode {
    pub const NONE_CODE: i32 = 0;

    pub fn code(self) -> i32 {
        self as i32
    }

    /// Reads a code that is allowed to say "no endpoint role at all".
    pub fn from_code(v: i32) -> Result<Option<Self>, EndpointModeError> {
        match v {
            Self::NONE_CODE => Ok(None),
            other => Self::try_from(other).map(Some),
        }
    }
}

impl From<EndpointMode> for i32 {
    fn from(value: EndpointMode) -> Self {
        value.code()
    }
}

impl TryFrom<i32> for EndpointMode {
    type Error = EndpointModeError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(EndpointMode::Game),
            2 => Ok(EndpointMode::Controller),
            _ => Err(EndpointModeError::OutOfRange(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum EndpointModeError {
    OutOfRange(i32),
}

impl std::fmt::Display for EndpointModeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EndpointModeError::OutOfRange(v) => write!(f, "EndpointMode out of range: {v}"),
        }
    }
}

impl std::error::Error for EndpointModeError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ActiveRoles {
    pub server: bool,
    pub endpoint: Option<EndpointMode>,
}

impl ActiveRoles {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn server_only() -> Self {
        Self {
            server: true,
            endpoint: None,
        }
    }

    pub fn game_only() -> Self {
        Self {
            server: false,
            endpoint: Some(EndpointMode::Game),
        }
    }

    pub fn controller_only() -> Self {
        Self {
            server: false,
            endpoint: Some(EndpointMode::Controller),
        }
    }

    pub fn game(&self) -> bool {
        self.endpoint == Some(EndpointMode::Game)
    }

    pub fn controller(&self) -> bool {
        self.endpoint == Some(EndpointMode::Controller)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_endpoint_role_survives_a_round_trip() {
        for mode in [EndpointMode::Game, EndpointMode::Controller] {
            assert_eq!(EndpointMode::from_code(mode.code()), Ok(Some(mode)));
        }
    }

    #[test]
    fn taking_no_endpoint_role_has_a_code_of_its_own() {
        assert_eq!(EndpointMode::from_code(EndpointMode::NONE_CODE), Ok(None));
    }

    #[test]
    fn an_unreadable_code_is_refused_rather_than_dropped() {
        for code in [-1, 3, 99] {
            assert_eq!(
                EndpointMode::from_code(code),
                Err(EndpointModeError::OutOfRange(code)),
                "{code} should not have been accepted"
            );
        }
    }

    #[test]
    fn a_role_crosses_a_binding_as_its_code() {
        for mode in [EndpointMode::Game, EndpointMode::Controller] {
            let bytes = rmp_serde::to_vec(&mode).expect("a role serialises");
            let as_code: i32 = rmp_serde::from_slice(&bytes).expect("as a plain number");
            assert_eq!(as_code, mode.code());

            let back: EndpointMode = rmp_serde::from_slice(&bytes).expect("and reads back");
            assert_eq!(back, mode);
        }
    }

    #[test]
    fn a_role_out_of_range_is_refused_at_the_boundary() {
        let bytes = rmp_serde::to_vec(&7i32).unwrap();
        assert!(rmp_serde::from_slice::<EndpointMode>(&bytes).is_err());
    }

    #[test]
    fn no_role_at_all_is_absence_rather_than_a_number() {
        let bytes = rmp_serde::to_vec(&None::<EndpointMode>).unwrap();
        let back: Option<EndpointMode> = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(back, None);
    }
}
