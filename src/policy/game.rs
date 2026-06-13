// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

//! Game-role policy: handlers for messages a game receives from a controller
//! (capabilities, control scheme requests, key/nav/menu, pause, buttons), plus
//! the client-reply handlers shared with the controller role. Holds the set of
//! scheme-defined button handler names used to recognise incoming button invokes.

use crate::engine::methods;
use crate::engine::processing::{Engine, RpcHandler};
use std::collections::HashSet;

#[derive(Debug, Default, Clone)]
pub struct GamePolicy {
    pub(crate) button_handlers: HashSet<String>,
}

impl GamePolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn claims(method: &str) -> Option<RpcHandler> {
        Some(match method {
            methods::DEFAULT_RETURN_REGISTER => Engine::rpc_on_register_reply,
            methods::ON_HOST_CONNECTED => Engine::rpc_host_slot_assigned,
            methods::CONNECTION_FAILED => Engine::rpc_connection_failed,
            methods::DEVICE_CONNECT_REQUESTED => Engine::rpc_device_connect_requested,
            methods::ON_KILL_EVENT => Engine::rpc_on_kill_event,
            methods::SET_CAPABILITIES => Engine::rpc_set_capabilities,
            methods::REQUEST_XML => Engine::rpc_request_xml,
            methods::ON_KEY_STRING => Engine::rpc_on_key_string,
            methods::ON_NAVIGATION_STRING => Engine::rpc_on_navigation_string,
            methods::MENU_EVENT => Engine::rpc_menu_event,
            methods::BM_PAUSE => Engine::rpc_bm_pause,
            methods::ON_CONTROL_SCHEME_PARSED => Engine::rpc_on_control_scheme_parsed,
            methods::GET_COOKIE => Engine::rpc_get_cookie,
            methods::SET_COOKIE => Engine::rpc_set_cookie,
            methods::GOT_COOKIE => Engine::rpc_got_cookie,
            _ => return None,
        })
    }
}
