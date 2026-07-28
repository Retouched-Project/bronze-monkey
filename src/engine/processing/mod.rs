// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

mod builders;
mod commands;
mod incoming;
mod rpc;
mod server_ops;

use crate::codec::messages::bm_encoding::Value;
use crate::devices::device_core::DeviceCore;
use crate::engine::device_registry::DeviceRegistry;
use crate::engine::events::{Event, ProcessOutput};
use crate::engine::state::EngineState;
use crate::policy::{ActiveRoles, ControllerPolicy, EndpointMode, GamePolicy, ServerPolicy};
use crate::types::channel_type::ChannelType;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ReceivedInvoke {
    pub method: String,
    pub return_method: Option<String>,
    pub params: Vec<Value>,
}

pub(crate) struct RpcContext<'a> {
    pub engine: &'a mut Engine,
    pub inv: &'a ReceivedInvoke,
    pub sender_id: Option<&'a str>,
    pub out: &'a mut ProcessOutput,
}

impl<'a> RpcContext<'a> {
    pub fn sender(&self) -> String {
        self.sender_id.unwrap_or_default().to_string()
    }

    pub fn param_str(&self, idx: usize) -> String {
        self.engine
            .param_string(&self.inv.params, idx)
            .unwrap_or_default()
    }

    pub fn param_i32(&self, idx: usize) -> Option<i32> {
        self.engine.param_i32(&self.inv.params, idx)
    }

    pub fn push_event(&mut self, event: Event) {
        self.out.events.push(event);
    }
}

pub(crate) type RpcHandler = fn(&mut RpcContext);

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
        crate::log_library_loaded();
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
        log::info!(
            "local device set: {} type={:?}",
            core.device_name,
            core.device_type
        );
        self.state.init_local_device(core);
    }

    pub fn roles(&self) -> ActiveRoles {
        self.roles
    }

    pub fn configure_roles(&mut self, server: bool, endpoint: Option<EndpointMode>) {
        log::info!("configure_roles server={server} endpoint={endpoint:?}");
        self.roles = ActiveRoles { server, endpoint };
    }

    pub fn set_server_role(&mut self, enabled: bool) {
        self.roles.server = enabled;
    }

    pub fn set_endpoint_mode(&mut self, mode: Option<EndpointMode>) {
        self.roles.endpoint = mode;
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
        if self.roles.game() {
            if let Some(handler) = GamePolicy::claims(method) {
                return Some(handler);
            }
        }
        if self.roles.controller() {
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

    pub(crate) fn reset_game_session(&mut self) {
        self.controller_policy.input_reliability = Default::default();
        self.state.chunk_buffers.clear();
    }

    pub(crate) fn set_input_reliability(&mut self, touch: Option<i32>, sensors: Option<i32>) {
        if let Some(touch) = touch {
            self.controller_policy.input_reliability.touch = Some(touch);
        }
        if let Some(sensors) = sensors {
            self.controller_policy.input_reliability.sensors = Some(sensors);
        }
    }

    pub fn reliability_for(&self, _target: &str, channel: i32) -> i32 {
        let tracked = &self.controller_policy.input_reliability;
        let requested = match ChannelType::from_i32(channel) {
            Some(ChannelType::Touch) => tracked.touch,
            Some(ChannelType::Acceleration | ChannelType::Gyro | ChannelType::Orientation) => {
                tracked.sensors
            }
            _ => None,
        };
        requested.unwrap_or_else(|| Self::default_reliability_for_channel(channel))
    }
}
