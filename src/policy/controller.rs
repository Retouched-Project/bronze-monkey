// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

//! Controller-role policy: handlers for messages a controller receives from a
//! game (vibrate, sensor/input configuration, host-list updates), plus the
//! client-reply handlers shared with the game role. Holds the session input
//! reliability the game requested via setReliabilityForTouch, applied when the
//! controller emits touch/sensor packets.

use crate::codec::externals::bm_reliability::BMReliability;
use crate::engine::methods;
use crate::engine::processing::{Engine, RpcHandler};

#[derive(Debug, Default, Clone, Copy)]
pub struct InputReliability {
    pub touch: Option<BMReliability>,
    pub sensors: Option<BMReliability>,
}

/// The screen a game is asked to lay a control scheme out for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub height: i32,
    pub width: i32,
}

impl Viewport {
    /// A screen is reported the way it is held upright, whichever way round it
    /// happened to be measured, so the longer side is always the height.
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            height: width.max(height),
            width: width.min(height),
        }
    }
}

/// What a controller tells a game about itself when a session opens, and
/// whether the engine says it. Held rather than sent on demand, so the order a
/// game needs is never the caller's problem.
#[derive(Debug, Default, Clone, Copy)]
pub struct SessionInputs {
    /// Whether the engine opens sessions itself. Off until asked, so an engine
    /// never speaks for a caller that did not ask it to.
    pub automatic: bool,
    pub gyroscope: bool,
    pub orientation: bool,
    pub viewport: Option<Viewport>,
}

#[derive(Debug, Default, Clone)]
pub struct ControllerPolicy {
    pub(crate) input_reliability: InputReliability,
    pub(crate) session: SessionInputs,
}

impl ControllerPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn claims(method: &str) -> Option<RpcHandler> {
        Some(match method {
            methods::DEFAULT_RETURN_REGISTER => Engine::rpc_on_register_reply,
            methods::DEFAULT_RETURN_LIST => Engine::rpc_on_list,
            methods::SET_RELIABILITY_FOR_TOUCH => Engine::rpc_set_reliability_for_touch,
            methods::ENABLE_ACCELEROMETER
            | methods::ENABLE_TOUCH
            | methods::SET_TOUCH_INTERVAL
            | methods::ENABLE_GYRO
            | methods::SET_GYRO_INTERVAL
            | methods::ENABLE_ORIENTATION
            | methods::SET_ORIENTATION_INTERVAL
            | methods::SET_CONTROL_MODE
            | methods::WAIT_FOR_NEW_HOST
            | methods::ON_PORTAL_ID => Engine::rpc_control_config,
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
