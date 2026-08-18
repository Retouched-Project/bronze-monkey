// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey
//
//! Engine configuration
//!
//! Everything a caller tells the engine about itself, in one place. State the
//! engine keeps for itself, or learns from a peer, is not here and is not the
//! caller's to set.
//!
//! The whole of it should be passed whenever any of it changes.
//! Roles are included because they move: an app that gains a built in server
//! says so the same way it says anything else.

use serde::{Deserialize, Serialize};

use crate::policy::EndpointMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct EngineConfig {
    pub server: bool,
    pub endpoint: Option<EndpointMode>,

    /// Whether the engine opens game sessions itself once a game acknowledges
    /// the connection. Needs a screen, since it asks for a control scheme.
    pub opens_sessions: bool,
    pub gyroscope: bool,
    pub orientation: bool,
    pub screen_width: i32,
    pub screen_height: i32,

    /// Whether a registry lets devices in without being asked.
    pub approves_registrations: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            server: false,
            endpoint: None,
            opens_sessions: true,
            gyroscope: false,
            orientation: false,
            screen_width: 0,
            screen_height: 0,
            approves_registrations: true,
        }
    }
}

impl EngineConfig {
    pub(crate) fn viewport(&self) -> Option<crate::policy::Viewport> {
        if self.screen_width > 0 && self.screen_height > 0 {
            Some(crate::policy::Viewport::new(
                self.screen_width,
                self.screen_height,
            ))
        } else {
            None
        }
    }

    pub(crate) fn check(&self) -> Result<(), ConfigError> {
        if self.endpoint == Some(EndpointMode::Controller)
            && self.opens_sessions
            && self.viewport().is_none()
        {
            return Err(ConfigError::ScreenRequired);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    ScreenRequired,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::ScreenRequired => write!(
                f,
                "a controller that opens its own sessions needs a screen to ask for a scheme for"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_screen_is_read_upright_either_way_round() {
        let upright = EngineConfig {
            screen_width: 1080,
            screen_height: 2151,
            ..Default::default()
        };
        let sideways = EngineConfig {
            screen_width: 2151,
            screen_height: 1080,
            ..Default::default()
        };
        assert_eq!(upright.viewport(), sideways.viewport());
    }

    #[test]
    fn a_controller_opening_its_own_sessions_needs_a_screen() {
        let config = EngineConfig {
            endpoint: Some(EndpointMode::Controller),
            opens_sessions: true,
            ..Default::default()
        };
        assert_eq!(config.check(), Err(ConfigError::ScreenRequired));
    }

    #[test]
    fn a_controller_opening_its_own_needs_nothing_else() {
        let config = EngineConfig {
            endpoint: Some(EndpointMode::Controller),
            opens_sessions: false,
            ..Default::default()
        };
        assert_eq!(config.check(), Ok(()));
    }

    #[test]
    fn a_server_needs_no_screen() {
        let config = EngineConfig {
            server: true,
            ..Default::default()
        };
        assert_eq!(config.check(), Ok(()));
    }
}
