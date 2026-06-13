// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

//! Controller-role policy: handlers for messages a controller receives from a
//! game (vibrate, pause, host-list updates), plus the client-reply handlers
//! shared with the game role.

use crate::engine::methods;
use crate::engine::processing::{Engine, RpcHandler};

#[derive(Debug, Clone, Default)]
pub struct ControllerPolicy {}

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
            methods::BM_PAUSE => Engine::rpc_bm_pause,
            methods::GET_COOKIE => Engine::rpc_get_cookie,
            methods::SET_COOKIE => Engine::rpc_set_cookie,
            methods::GOT_COOKIE => Engine::rpc_got_cookie,
            _ => return None,
        })
    }
}
