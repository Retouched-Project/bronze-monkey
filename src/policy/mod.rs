// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use serde::{Deserialize, Serialize};

pub mod controller;
pub mod game;
pub mod server;

pub use controller::{ControllerPolicy, InputReliability};
pub use game::GamePolicy;
pub use server::ServerPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EndpointMode {
    Game,
    Controller,
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
