// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use super::{Engine, ReceivedInvoke, RpcContext};
use crate::codec::externals::bm_array::BMArray;
use crate::codec::externals::bm_registry_info::BMRegistryInfo;
use crate::codec::messages::bm_encoding::Value;
use crate::codec::object::Object;
use crate::engine::events::{ControlConfig, Event};
use crate::engine::methods;
use crate::policy::server::PendingRegistration;
use crate::types::control_mode::ControlMode;
use crate::types::device_type::DeviceType;

impl Engine {
    pub(crate) fn rpc_registry_register(ctx: &mut RpcContext) {
        let sender_id = ctx.sender_id;
        let RpcContext {
            engine, inv, out, ..
        } = ctx;
        let infos = engine.collect_registry_infos(&inv.params);
        let domain = inv
            .params
            .iter()
            .find_map(|p| match engine.unwrap_value(p) {
                Value::String(s) => Some(s.clone()),
                _ => None,
            });

        let Some(target_id) = sender_id else {
            log::warn!("registry.register missing sender id");
            return;
        };

        let Some(mut info) = infos.first().cloned() else {
            return;
        };

        if engine.server_policy.auto_approve_registration {
            let is_game = matches!(
                info.device.device_type,
                DeviceType::Flash | DeviceType::Unity | DeviceType::Native
            );
            if is_game {
                let dev_id = info.device.device_id.clone();
                if let Some(existing) = engine
                    .state
                    .registry
                    .get(&dev_id)
                    .and_then(|r| r.info.as_ref().map(|i| i.slot_id))
                {
                    if existing > 0 {
                        engine.state.used_slots.remove(&existing);
                    }
                }
                info.slot_id = engine.state.allocate_slot();
            } else {
                info.slot_id = 0;
            }
            engine.state.upsert_registry_info(info.clone());

            if let Some(reply) = Self::reply_method(inv.return_method.as_deref()) {
                out.outgoings.extend(engine.make_message_invoke(
                    target_id,
                    reply,
                    None,
                    vec![Value::Bool(true)],
                ));
            } else {
                log::warn!(
                    "registry.register from '{target_id}' omitted a return method, skipping reply"
                );
            }
            out.events.push(Event::PeerRegistered {
                info: info.clone(),
                domain,
                success: true,
            });

            if is_game {
                let info_val = Value::Object(Object::BMRegistryInfo(info.clone()));

                out.outgoings.extend(engine.make_message_invoke(
                    target_id,
                    methods::ON_HOST_CONNECTED,
                    None,
                    vec![info_val.clone()],
                ));

                let viewer_ids: Vec<String> = engine
                    .state
                    .registry
                    .snapshot()
                    .into_iter()
                    .filter_map(|r| r.info)
                    .filter(|r| {
                        !matches!(
                            r.device.device_type,
                            DeviceType::Flash
                                | DeviceType::Unity
                                | DeviceType::Native
                                | DeviceType::Server
                        )
                    })
                    .filter(|r| r.device.device_id != target_id)
                    .map(|r| r.device.device_id)
                    .collect();
                for vid in viewer_ids {
                    out.outgoings.extend(engine.make_message_invoke(
                        &vid,
                        methods::ON_HOST_CONNECTED,
                        None,
                        vec![info_val.clone()],
                    ));
                }
            }
        } else {
            // Manual approval: stash the request (with the caller's return
            // method) until the integrator calls approve/deny_registration.
            engine.server_policy.pending_registrations.insert(
                info.device.device_id.clone(),
                PendingRegistration {
                    info,
                    target_id: target_id.to_string(),
                    return_method: inv.return_method.clone(),
                },
            );
        }
    }

    pub(crate) fn rpc_on_register_reply(ctx: &mut RpcContext) {
        let RpcContext {
            engine, inv, out, ..
        } = ctx;
        let success = inv.params.iter().find_map(|p| {
            if let Value::Bool(b) = engine.unwrap_value(p) {
                Some(*b)
            } else {
                None
            }
        });
        if let Some(success) = success {
            out.events.push(Event::RegistrationResult { success });
        }
    }

    pub(crate) fn rpc_registry_list(ctx: &mut RpcContext) {
        let sender_id = ctx.sender_id;
        let RpcContext {
            engine, inv, out, ..
        } = ctx;
        // Server side: answer the list request via the caller's return method.
        let Some(target_id) = sender_id else {
            return;
        };
        let Some(reply) = Self::reply_method(inv.return_method.as_deref()) else {
            log::warn!(
                "registry.list from '{target_id}' omitted a return method, not replying with host list"
            );
            return;
        };

        let list_infos = engine
            .state
            .visible_host_infos(&engine.server_policy.hidden_hosts);
        let mut arr = BMArray::default();
        for r in list_infos {
            arr.push(Value::Object(Object::BMRegistryInfo(r)));
        }
        out.outgoings.extend(engine.make_message_invoke(
            target_id,
            reply,
            None,
            vec![Value::Object(Object::BMArray(arr))],
        ));
    }

    pub(crate) fn rpc_on_list(ctx: &mut RpcContext) {
        let RpcContext {
            engine, inv, out, ..
        } = ctx;
        let infos = engine.collect_registry_infos(&inv.params);
        out.events.push(Event::HostList { infos });
    }

    pub(crate) fn rpc_registry_relay(ctx: &mut RpcContext) {
        let sender_id = ctx.sender_id;
        let RpcContext {
            engine, inv, out, ..
        } = ctx;
        let mut target_id = None;
        let mut relayed = None;

        for p in &inv.params {
            match engine.unwrap_value(p) {
                Value::Object(Object::BMRegistryInfo(r)) => {
                    target_id = Some(r.device.device_id.clone());
                }
                Value::Object(Object::BMInvoke(bm_invoke)) => {
                    relayed = Some(bm_invoke.clone());
                }
                _ => {}
            }
        }

        let Some(target_id) = target_id else {
            return;
        };
        let Some(relayed) = relayed else {
            return;
        };

        out.events.push(Event::Relayed {
            sender: sender_id.map(|s| s.to_string()),
            destination: target_id.clone(),
            method: relayed.method.clone(),
            return_method: relayed.return_method.clone(),
            params: relayed.params.clone(),
        });

        out.outgoings.extend(engine.make_message_invoke(
            &target_id,
            &relayed.method,
            relayed.return_method.as_deref(),
            relayed.params,
        ));
    }

    pub(crate) fn rpc_on_host_connected(ctx: &mut RpcContext) {
        let RpcContext {
            engine, inv, out, ..
        } = ctx;
        for info in engine.collect_registry_infos(&inv.params) {
            out.events.push(Event::HostConnected { info });
        }
    }

    pub(crate) fn rpc_host_slot_assigned(ctx: &mut RpcContext) {
        let RpcContext {
            engine, inv, out, ..
        } = ctx;
        let local = engine.local_device_id();
        for info in engine.collect_registry_infos(&inv.params) {
            if info.device.device_id == local {
                out.events.push(Event::SlotAssigned { info });
            }
        }
    }

    pub(crate) fn rpc_registry_update(ctx: &mut RpcContext) {
        let sender_id = ctx.sender_id;
        let RpcContext {
            engine, inv, out, ..
        } = ctx;
        let infos = engine.collect_registry_infos(&inv.params);
        for info in &infos {
            out.events.push(Event::HostUpdated { info: info.clone() });
        }

        if infos.is_empty() {
            return;
        }

        let viewer_ids: Vec<String> = engine
            .state
            .registry
            .snapshot()
            .into_iter()
            .filter_map(|r| r.info)
            .filter(|r| {
                !matches!(
                    r.device.device_type,
                    DeviceType::Flash | DeviceType::Unity | DeviceType::Native
                )
            })
            .map(|r| r.device.device_id)
            .collect();

        for info in infos.into_iter() {
            engine.state.upsert_registry_info(info.clone());
            if !matches!(
                info.device.device_type,
                DeviceType::Flash | DeviceType::Unity | DeviceType::Native
            ) {
                continue;
            }
            if engine
                .server_policy
                .hidden_hosts
                .contains(&info.device.device_id)
            {
                continue;
            }
            let Some(stored) = engine
                .state
                .registry
                .get(&info.device.device_id)
                .and_then(|r| r.info.clone())
            else {
                continue;
            };
            for vid in &viewer_ids {
                out.outgoings.extend(engine.make_message_invoke(
                    vid,
                    methods::ON_HOST_UPDATE,
                    None,
                    vec![Value::Object(Object::BMRegistryInfo(stored.clone()))],
                ));
            }
        }

        if let (Some(target_id), Some(reply)) =
            (sender_id, Self::reply_method(inv.return_method.as_deref()))
        {
            out.outgoings.extend(engine.make_message_invoke(
                target_id,
                reply,
                None,
                vec![Value::Bool(true)],
            ));
        }
    }

    pub(crate) fn rpc_host_updated(ctx: &mut RpcContext) {
        let RpcContext {
            engine, inv, out, ..
        } = ctx;
        for info in engine.collect_registry_infos(&inv.params) {
            out.events.push(Event::HostUpdated { info });
        }
    }

    pub(crate) fn rpc_on_host_disconnected(ctx: &mut RpcContext) {
        let RpcContext {
            engine, inv, out, ..
        } = ctx;
        for info in engine.collect_registry_infos(&inv.params) {
            out.events.push(Event::HostDisconnected { info });
        }
    }

    pub(crate) fn rpc_device_connect_requested(ctx: &mut RpcContext) {
        let RpcContext {
            engine, inv, out, ..
        } = ctx;
        for info in engine.collect_registry_infos(&inv.params) {
            out.events.push(Event::DeviceConnectRequested { info });
        }
    }

    pub(crate) fn rpc_on_kill_event(ctx: &mut RpcContext) {
        ctx.push_event(Event::DeviceKilled {
            device_id: ctx.param_str(0),
        });
    }

    pub(crate) fn rpc_connection_failed(ctx: &mut RpcContext) {
        ctx.push_event(Event::ConnectionFailed {
            device_id: ctx.param_str(0),
        });
    }

    pub(crate) fn rpc_vibrate(ctx: &mut RpcContext) {
        ctx.push_event(Event::Vibrate {
            sender: ctx.sender(),
        });
    }

    pub(crate) fn rpc_bm_pause(ctx: &mut RpcContext) {
        ctx.push_event(Event::Pause {
            sender: ctx.sender(),
        });
    }

    pub(crate) fn rpc_menu_event(ctx: &mut RpcContext) {
        let event = ctx.param_str(0);
        ctx.push_event(Event::MenuEvent {
            sender: ctx.sender(),
            event,
        });
    }

    pub(crate) fn rpc_on_key_string(ctx: &mut RpcContext) {
        let key = ctx.param_str(0);
        ctx.push_event(Event::KeyString {
            sender: ctx.sender(),
            key,
        });
    }

    pub(crate) fn rpc_on_navigation_string(ctx: &mut RpcContext) {
        let nav = ctx.param_str(0);
        ctx.push_event(Event::Navigation {
            sender: ctx.sender(),
            nav,
        });
    }

    pub(crate) fn rpc_set_capabilities(ctx: &mut RpcContext) {
        let mask = ctx.param_i32(0).unwrap_or(0);
        ctx.push_event(Event::Capabilities {
            sender: ctx.sender(),
            gyroscope: mask & 1 != 0,
            orientation: mask & 2 != 0,
        });
    }

    /// A controller asks this on every connection so it knows where to send the
    /// player back to. A game that was not launched from anywhere answers with
    /// nothing, which is the answer for every game that stands on its own.
    pub(crate) fn rpc_get_portal_id(ctx: &mut RpcContext) {
        let sender_id = ctx.sender_id;
        let RpcContext {
            engine, inv, out, ..
        } = ctx;
        let Some(target_id) = sender_id else {
            return;
        };
        let Some(reply) = Self::reply_method(inv.return_method.as_deref()) else {
            return;
        };
        out.outgoings.extend(engine.make_message_invoke(
            target_id,
            reply,
            None,
            vec![Value::String(String::new())],
        ));
    }

    pub(crate) fn rpc_request_xml(ctx: &mut RpcContext) {
        let height = ctx.param_i32(0).unwrap_or(0);
        let width = ctx.param_i32(1).unwrap_or(0);
        let requester = ctx.param_str(2);
        ctx.push_event(Event::ControlSchemeRequested {
            sender: ctx.sender(),
            width,
            height,
            requester,
        });
    }

    pub(crate) fn rpc_on_control_scheme_parsed(ctx: &mut RpcContext) {
        let device_id = ctx.param_str(0);
        ctx.push_event(Event::ControlSchemeParsed {
            sender: ctx.sender(),
            device_id,
        });
    }

    pub(crate) fn rpc_get_cookie(ctx: &mut RpcContext) {
        let name = ctx.param_str(0);
        ctx.push_event(Event::CookieRequested {
            sender: ctx.sender(),
            name,
        });
    }

    pub(crate) fn rpc_set_cookie(ctx: &mut RpcContext) {
        let name = ctx.param_str(0);
        let value = ctx.param_str(1);
        ctx.push_event(Event::CookieStored {
            sender: ctx.sender(),
            name,
            value,
        });
    }

    pub(crate) fn rpc_got_cookie(ctx: &mut RpcContext) {
        let name = ctx.param_str(0);
        let value = ctx.param_str(1);
        ctx.push_event(Event::Cookie {
            sender: ctx.sender(),
            name,
            value,
        });
    }

    pub(crate) fn rpc_registry_remove(ctx: &mut RpcContext) {
        let sender_id = ctx.sender_id;
        let RpcContext {
            engine, inv, out, ..
        } = ctx;
        let device_id = engine
            .param_string(&inv.params, 0)
            .or_else(|| sender_id.map(|s| s.to_string()));
        let Some(device_id) = device_id else {
            return;
        };
        if engine.state.registry.get(&device_id).is_none() {
            return;
        }
        if let (Some(target_id), Some(reply)) =
            (sender_id, Self::reply_method(inv.return_method.as_deref()))
        {
            out.outgoings.extend(engine.make_message_invoke(
                target_id,
                reply,
                None,
                vec![Value::Bool(true)],
            ));
        }
        out.outgoings.extend(engine.drop_device(&device_id));
    }

    pub(crate) fn rpc_registry_set_visible(ctx: &mut RpcContext) {
        let sender_id = ctx.sender_id;
        let RpcContext {
            engine, inv, out, ..
        } = ctx;
        let Some(target_id) = sender_id else {
            return;
        };
        let Some(visible) = engine.param_bool(&inv.params, 0) else {
            return;
        };
        let notify_everyone = engine.param_bool(&inv.params, 1).unwrap_or(false);

        let device_id = target_id.to_string();
        let changed = if visible {
            engine.server_policy.hidden_hosts.remove(&device_id)
        } else {
            engine.server_policy.hidden_hosts.insert(device_id.clone())
        };

        if changed && notify_everyone {
            if let Some(info) = engine
                .state
                .registry
                .get(&device_id)
                .and_then(|r| r.info.clone())
            {
                let is_game = matches!(
                    info.device.device_type,
                    DeviceType::Flash | DeviceType::Unity | DeviceType::Native
                );
                if is_game {
                    let method = if visible {
                        methods::ON_HOST_CONNECTED
                    } else {
                        methods::ON_HOST_DISCONNECTED
                    };
                    let info_val = Value::Object(Object::BMRegistryInfo(info));
                    let viewer_ids: Vec<String> = engine
                        .state
                        .registry
                        .snapshot()
                        .into_iter()
                        .filter_map(|r| r.info)
                        .filter(|r| {
                            !matches!(
                                r.device.device_type,
                                DeviceType::Flash
                                    | DeviceType::Unity
                                    | DeviceType::Native
                                    | DeviceType::Server
                            )
                        })
                        .map(|r| r.device.device_id)
                        .collect();
                    for vid in viewer_ids {
                        out.outgoings.extend(engine.make_message_invoke(
                            &vid,
                            method,
                            None,
                            vec![info_val.clone()],
                        ));
                    }
                }
            }
        }

        if let Some(reply) = Self::reply_method(inv.return_method.as_deref()) {
            out.outgoings.extend(engine.make_message_invoke(
                target_id,
                reply,
                None,
                vec![Value::Bool(true)],
            ));
        }
    }

    fn unwrap_value<'a>(&self, v: &'a Value) -> &'a Value {
        if let Value::Object(Object::BMParameter(inner)) = v {
            &inner.value
        } else {
            v
        }
    }

    fn collect_registry_infos(&self, params: &[Value]) -> Vec<BMRegistryInfo> {
        let mut out = Vec::new();
        for p in params {
            let val = self.unwrap_value(p);
            match val {
                Value::Object(Object::BMRegistryInfo(r)) => out.push(r.clone()),
                Value::Object(Object::BMArray(arr)) => {
                    for v in arr.items.iter() {
                        let inner_val = self.unwrap_value(v);
                        if let Value::Object(Object::BMRegistryInfo(r)) = inner_val {
                            out.push(r.clone());
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    pub(super) fn parse_control_rpc(&self, inv: &ReceivedInvoke) -> Option<ControlConfig> {
        let mut touch_enabled = None;
        let mut accel_enabled = None;
        let mut gyro_enabled = None;
        let mut orientation_enabled = None;
        let mut touch_interval_ms = None;
        let mut accel_interval_ms = None;
        let mut gyro_interval_ms = None;
        let mut orientation_interval_ms = None;
        let mut control_mode = None;
        let mut portal_id = None;
        let mut return_app_id = None;
        let mut start_string = None;

        match inv.method.as_str() {
            methods::ENABLE_ACCELEROMETER => {
                touch_enabled = None;
                accel_enabled = self.param_bool(&inv.params, 0);
                if let Some(sec) = self.param_f64(&inv.params, 1) {
                    accel_interval_ms = Some((sec * 1000.0) as i32);
                }
            }
            methods::ENABLE_TOUCH => {
                touch_enabled = self.param_bool(&inv.params, 0);
            }
            methods::SET_TOUCH_INTERVAL => {
                if let Some(sec) = self.param_f64(&inv.params, 0) {
                    touch_interval_ms = Some((sec * 1000.0) as i32);
                }
            }
            methods::ENABLE_GYRO => {
                gyro_enabled = self.param_bool(&inv.params, 0);
            }
            methods::SET_GYRO_INTERVAL => {
                if let Some(sec) = self.param_f64(&inv.params, 0) {
                    gyro_interval_ms = Some((sec * 1000.0) as i32);
                }
            }
            methods::ENABLE_ORIENTATION => {
                orientation_enabled = self.param_bool(&inv.params, 0);
            }
            methods::SET_ORIENTATION_INTERVAL => {
                if let Some(sec) = self.param_f64(&inv.params, 0) {
                    orientation_interval_ms = Some((sec * 1000.0) as i32);
                }
            }
            methods::SET_CONTROL_MODE => {
                control_mode = self
                    .param_i32(&inv.params, 0)
                    .and_then(ControlMode::from_wire);
                start_string = self.param_string(&inv.params, 1);
            }
            methods::WAIT_FOR_NEW_HOST => {
                portal_id = self.param_string(&inv.params, 0);
                control_mode = Some(ControlMode::Wait);
            }
            methods::ON_PORTAL_ID => {
                return_app_id = self.param_string(&inv.params, 0);
            }
            _ => return None,
        }

        Some(ControlConfig {
            touch_enabled,
            accel_enabled,
            gyro_enabled,
            orientation_enabled,
            touch_interval_ms,
            accel_interval_ms,
            gyro_interval_ms,
            orientation_interval_ms,
            control_mode,
            portal_id,
            return_app_id,
            start_string,
        })
    }

    fn param_bool(&self, params: &[Value], idx: usize) -> Option<bool> {
        match params.get(idx)? {
            Value::Bool(b) => Some(*b),
            Value::I16(v) => Some(*v != 0),
            Value::I32(v) => Some(*v != 0),
            Value::U16(v) => Some(*v != 0),
            Value::U32(v) => Some(*v != 0),
            _ => None,
        }
    }

    pub(super) fn param_i32(&self, params: &[Value], idx: usize) -> Option<i32> {
        match params.get(idx)? {
            Value::I16(v) => Some(*v as i32),
            Value::I32(v) => Some(*v),
            Value::U16(v) => Some(*v as i32),
            Value::U32(v) => Some(*v as i32),
            _ => None,
        }
    }

    fn param_f64(&self, params: &[Value], idx: usize) -> Option<f64> {
        match params.get(idx)? {
            Value::F32(v) => Some(*v as f64),
            Value::F64(v) => Some(*v),
            Value::I16(v) => Some(*v as f64),
            Value::I32(v) => Some(*v as f64),
            Value::U16(v) => Some(*v as f64),
            Value::U32(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub(super) fn param_string(&self, params: &[Value], idx: usize) -> Option<String> {
        match params.get(idx)? {
            Value::String(s) => Some(s.clone()),
            _ => None,
        }
    }
}
