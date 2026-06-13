// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

//! Server-role policy: registry RPC handlers, approve/deny/drop, viewer broadcast.
//! Holds state that is meaningful only when the engine is acting as a server.

use crate::codec::externals::bm_registry_info::BMRegistryInfo;
use crate::engine::methods;
use crate::engine::processing::{Engine, RpcHandler};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub(crate) struct PendingRegistration {
    pub info: BMRegistryInfo,
    pub target_id: String,
    pub return_method: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ServerPolicy {
    pub auto_approve_registration: bool,
    pub(crate) pending_registrations: HashMap<String, PendingRegistration>,
    pub(crate) hidden_hosts: HashSet<String>,
}

impl Default for ServerPolicy {
    fn default() -> Self {
        Self {
            auto_approve_registration: true,
            pending_registrations: HashMap::new(),
            hidden_hosts: HashSet::new(),
        }
    }
}

impl ServerPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn claims(method: &str) -> Option<RpcHandler> {
        Some(match method {
            methods::REGISTRY_REGISTER => Engine::rpc_registry_register,
            methods::REGISTRY_LIST => Engine::rpc_registry_list,
            methods::REGISTRY_RELAY => Engine::rpc_registry_relay,
            methods::REGISTRY_UPDATE => Engine::rpc_registry_update,
            methods::REGISTRY_REMOVE => Engine::rpc_registry_remove,
            methods::REGISTRY_SET_VISIBLE => Engine::rpc_registry_set_visible,
            _ => return None,
        })
    }
}
