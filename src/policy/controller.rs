// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

//! Controller-role policy: handlers for messages a controller receives from a
//! game (vibrate, sensor/input configuration, host-list updates), plus the
//! client-reply handlers shared with the game role. Holds the per-game input
//! reliability the game requested via setReliabilityForTouch, applied when the
//! controller emits touch/sensor packets.

use crate::engine::methods;
use crate::engine::processing::{Engine, RpcHandler};
use std::collections::HashMap;

#[derive(Debug, Default, Clone, Copy)]
pub struct InputReliability {
    pub touch: Option<i32>,
    pub sensors: Option<i32>,
}

#[derive(Debug, Default, Clone)]
pub struct ControllerPolicy {
    pub(crate) input_reliability: HashMap<String, InputReliability>,
}

impl ControllerPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn claims(method: &str) -> Option<RpcHandler> {
        Some(match method {
            methods::DEFAULT_RETURN_REGISTER => Engine::rpc_on_register_reply,
            methods::DEFAULT_RETURN_LIST => Engine::rpc_on_list,
            methods::ON_HOST_CONNECTED => Engine::rpc_on_host_connected,
            methods::ON_HOST_UPDATE => Engine::rpc_host_updated,
            methods::ON_HOST_DISCONNECTED => Engine::rpc_on_host_disconnected,
            methods::CONNECTION_FAILED => Engine::rpc_connection_failed,
            methods::VIBRATE => Engine::rpc_vibrate,
            methods::GET_COOKIE => Engine::rpc_get_cookie,
            methods::SET_COOKIE => Engine::rpc_set_cookie,
            methods::GOT_COOKIE => Engine::rpc_got_cookie,
            _ => return None,
        })
    }
}
