// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

mod builders;
mod commands;
mod incoming;
mod rpc;
mod server_ops;

use crate::codec::messages::bm_encoding::Value;
use crate::devices::device_core::DeviceCore;
use crate::engine::events::{ControlConfig, ProcessOutput};
use crate::engine::registry::DeviceRegistry;
use crate::engine::state::EngineState;
use crate::policy::{ActiveRoles, ControllerPolicy, GamePolicy, Role, ServerPolicy};
use crate::types::channel_type::ChannelType;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ReceivedInvoke {
    pub method: String,
    pub return_method: Option<String>,
    pub params: Vec<Value>,
}

pub(crate) type RpcHandler =
    fn(&mut Engine, &ReceivedInvoke, Option<&str>, i32, &mut ProcessOutput);

#[derive(Debug, Default, Clone)]
pub struct Engine {
    pub(crate) state: EngineState,
    pub(crate) roles: ActiveRoles,
    bound_continuations: HashMap<String, RpcHandler>,
    pub server_policy: ServerPolicy,
    pub game_policy: GamePolicy,
    pub controller_policy: ControllerPolicy,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            state: EngineState::new(),
            roles: ActiveRoles::default(),
            bound_continuations: HashMap::new(),
            server_policy: ServerPolicy::new(),
            game_policy: GamePolicy::new(),
            controller_policy: ControllerPolicy::new(),
        }
    }

    pub fn init_local_device(&mut self, core: DeviceCore) {
        self.roles = ActiveRoles::for_device_type(core.device_type);
        self.state.init_local_device(core);
    }

    pub fn roles(&self) -> ActiveRoles {
        self.roles
    }

    pub fn set_role_enabled(&mut self, role: Role, enabled: bool) {
        self.roles.set(role, enabled);
    }

    pub(crate) fn resolve_handler(&self, method: &str) -> Option<RpcHandler> {
        if let Some(handler) = self.bound_continuations.get(method) {
            return Some(*handler);
        }
        if self.roles.server {
            if let Some(handler) = ServerPolicy::claims(method) {
                return Some(handler);
            }
        }
        if self.roles.game {
            if let Some(handler) = GamePolicy::claims(method) {
                return Some(handler);
            }
        }
        if self.roles.controller {
            if let Some(handler) = ControllerPolicy::claims(method) {
                return Some(handler);
            }
        }
        None
    }

    fn reply_method(return_method: Option<&str>) -> Option<&str> {
        return_method.filter(|m| !m.is_empty())
    }

    fn return_method_or<'a>(requested: Option<&'a str>, default: &'a str) -> &'a str {
        match requested {
            Some(m) if !m.is_empty() => m,
            _ => default,
        }
    }

    fn local_device_id(&self) -> String {
        self.state
            .local_device
            .as_ref()
            .map(|d| d.device_id.clone())
            .unwrap_or_default()
    }

    pub fn registry(&self) -> &DeviceRegistry {
        &self.state.registry
    }

    pub fn registry_mut(&mut self) -> &mut DeviceRegistry {
        &mut self.state.registry
    }

    pub fn register_button_handlers<I>(&mut self, handlers: I)
    where
        I: IntoIterator,
        I::Item: Into<String>,
    {
        self.game_policy
            .button_handlers
            .extend(handlers.into_iter().map(Into::into));
    }

    pub fn clear_button_handlers(&mut self) {
        self.game_policy.button_handlers.clear();
    }

    fn bind_continuation(&mut self, return_method: &str, handler: RpcHandler) {
        if return_method.is_empty() || self.resolve_handler(return_method).is_some() {
            return;
        }
        self.bound_continuations
            .insert(return_method.to_string(), handler);
    }

    fn track_reliability(&mut self, sender: &str, cfg: &ControlConfig) {
        if cfg.touch_reliability.is_none() && cfg.control_reliability.is_none() {
            return;
        }
        let entry = self
            .controller_policy
            .input_reliability
            .entry(sender.to_string())
            .or_default();
        if let Some(touch) = cfg.touch_reliability {
            entry.touch = Some(touch);
        }
        if let Some(sensors) = cfg.control_reliability {
            entry.sensors = Some(sensors);
        }
    }

    pub fn reliability_for(&self, target: &str, channel: i32) -> i32 {
        let tracked = self.controller_policy.input_reliability.get(target);
        let requested = match ChannelType::from_i32(channel) {
            Some(ChannelType::Touch) => tracked.and_then(|r| r.touch),
            Some(ChannelType::Acceleration | ChannelType::Gyro | ChannelType::Orientation) => {
                tracked.and_then(|r| r.sensors)
            }
            _ => None,
        };
        requested.unwrap_or_else(|| Self::default_reliability_for_channel(channel))
    }
}
