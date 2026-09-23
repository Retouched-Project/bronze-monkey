// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::types::coded::crosses_as_its_code;
use serde::{Deserialize, Serialize};

pub mod controller;
pub mod game;
pub mod server;

pub use controller::{ControllerPolicy, InputReliability, SessionInputs, Viewport};
pub use game::GamePolicy;
pub use server::ServerPolicy;

crosses_as_its_code! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum EndpointMode {
        Game = 1,
        Controller = 2,
    }
}

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
    fn no_role_at_all_is_absence_rather_than_a_number() {
        let bytes = rmp_serde::to_vec(&None::<EndpointMode>).unwrap();
        let back: Option<EndpointMode> = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(back, None);

        let zero = rmp_serde::to_vec(&0).unwrap();
        assert!(rmp_serde::from_slice::<Option<EndpointMode>>(&zero).is_err());
    }
}
