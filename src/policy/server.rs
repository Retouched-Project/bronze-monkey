// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

//! Server-role policy: registry RPC handlers, approve/deny/drop, viewer broadcast.
//! Holds state that is meaningful only when the engine is acting as a server.

use crate::codec::externals::bm_registry_info::BMRegistryInfo;
use std::collections::HashMap;

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
}

impl Default for ServerPolicy {
    fn default() -> Self {
        Self {
            auto_approve_registration: true,
            pending_registrations: HashMap::new(),
        }
    }
}

impl ServerPolicy {
    pub fn new() -> Self {
        Self::default()
    }
}
