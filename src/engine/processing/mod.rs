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
use crate::engine::methods;
use crate::engine::registry::DeviceRegistry;
use crate::engine::state::EngineState;
use crate::types::channel_type::ChannelType;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ReceivedInvoke {
    pub method: String,
    pub return_method: Option<String>,
    pub params: Vec<Value>,
}

type RpcHandler = fn(&mut Engine, &ReceivedInvoke, Option<&str>, i32, &mut ProcessOutput);

#[derive(Debug, Default, Clone)]
pub struct Engine {
    pub(crate) state: EngineState,
    rpc_handlers: HashMap<String, RpcHandler>,
    pub server_policy: crate::policy::ServerPolicy,
}

impl Engine {
    pub fn new() -> Self {
        let mut handlers: HashMap<String, RpcHandler> = HashMap::with_capacity(32);
        handlers.insert(
            methods::REGISTRY_REGISTER.to_string(),
            Self::rpc_registry_register,
        );
        handlers.insert(
            methods::DEFAULT_RETURN_REGISTER.to_string(),
            Self::rpc_registry_register,
        );
        handlers.insert(methods::REGISTRY_LIST.to_string(), Self::rpc_registry_list);
        handlers.insert(
            methods::DEFAULT_RETURN_LIST.to_string(),
            Self::rpc_registry_list,
        );
        handlers.insert(
            methods::REGISTRY_RELAY.to_string(),
            Self::rpc_registry_relay,
        );
        handlers.insert(
            methods::ON_HOST_CONNECTED.to_string(),
            Self::rpc_on_host_connected,
        );
        handlers.insert(
            methods::REGISTRY_UPDATE.to_string(),
            Self::rpc_registry_update,
        );
        handlers.insert(
            methods::ON_HOST_UPDATE.to_string(),
            Self::rpc_registry_update,
        );
        handlers.insert(
            methods::ON_HOST_DISCONNECTED.to_string(),
            Self::rpc_on_host_disconnected,
        );
        handlers.insert(
            methods::DEVICE_CONNECT_REQUESTED.to_string(),
            Self::rpc_device_connect_requested,
        );
        handlers.insert(
            methods::CONNECTION_FAILED.to_string(),
            Self::rpc_connection_failed,
        );
        handlers.insert(methods::VIBRATE.to_string(), Self::rpc_vibrate);
        handlers.insert(methods::BM_PAUSE.to_string(), Self::rpc_bm_pause);
        handlers.insert(methods::MENU_EVENT.to_string(), Self::rpc_menu_event);
        handlers.insert(methods::ON_KEY_STRING.to_string(), Self::rpc_on_key_string);
        handlers.insert(
            methods::ON_NAVIGATION_STRING.to_string(),
            Self::rpc_on_navigation_string,
        );
        handlers.insert(
            methods::SET_CAPABILITIES.to_string(),
            Self::rpc_set_capabilities,
        );
        handlers.insert(methods::REQUEST_XML.to_string(), Self::rpc_request_xml);
        handlers.insert(
            methods::ON_CONTROL_SCHEME_PARSED.to_string(),
            Self::rpc_on_control_scheme_parsed,
        );
        handlers.insert(methods::GET_COOKIE.to_string(), Self::rpc_get_cookie);
        handlers.insert(methods::SET_COOKIE.to_string(), Self::rpc_set_cookie);
        handlers.insert(methods::GOT_COOKIE.to_string(), Self::rpc_got_cookie);
        handlers.insert(
            methods::REGISTRY_REMOVE.to_string(),
            Self::rpc_registry_remove,
        );
        handlers.insert(
            methods::REGISTRY_SET_VISIBLE.to_string(),
            Self::rpc_registry_set_visible,
        );

        Self {
            state: EngineState::new(),
            rpc_handlers: handlers,
            server_policy: crate::policy::ServerPolicy::new(),
        }
    }

    pub fn init_local_device(&mut self, core: DeviceCore) {
        self.state.init_local_device(core);
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
        self.state
            .button_handlers
            .extend(handlers.into_iter().map(Into::into));
    }

    pub fn clear_button_handlers(&mut self) {
        self.state.button_handlers.clear();
    }

    fn bind_continuation(&mut self, return_method: &str, handler: RpcHandler) {
        if return_method.is_empty() || self.rpc_handlers.contains_key(return_method) {
            return;
        }
        self.rpc_handlers.insert(return_method.to_string(), handler);
    }

    fn track_reliability(&mut self, sender: &str, cfg: &ControlConfig) {
        if cfg.touch_reliability.is_none() && cfg.control_reliability.is_none() {
            return;
        }
        let entry = self
            .state
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
        let tracked = self.state.input_reliability.get(target);
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
