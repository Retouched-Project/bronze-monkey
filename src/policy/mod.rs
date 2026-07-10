// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

pub mod controller;
pub mod game;
pub mod server;

pub use controller::{ControllerPolicy, InputReliability};
pub use game::GamePolicy;
pub use server::ServerPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Server,
    Game,
    Controller,
}

#[derive(Debug, Clone, Copy)]
pub struct ActiveRoles {
    pub server: bool,
    pub game: bool,
    pub controller: bool,
}

impl ActiveRoles {
    pub fn all() -> Self {
        Self {
            server: true,
            game: true,
            controller: true,
        }
    }

    pub fn none() -> Self {
        Self {
            server: false,
            game: false,
            controller: false,
        }
    }

    pub fn has(&self, role: Role) -> bool {
        match role {
            Role::Server => self.server,
            Role::Game => self.game,
            Role::Controller => self.controller,
        }
    }

    pub fn set(&mut self, role: Role, enabled: bool) {
        match role {
            Role::Server => self.server = enabled,
            Role::Game => self.game = enabled,
            Role::Controller => self.controller = enabled,
        }
    }
}

impl Default for ActiveRoles {
    fn default() -> Self {
        Self::none()
    }
}
