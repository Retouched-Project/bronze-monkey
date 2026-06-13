// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use std::os::raw::c_uchar;

use crate::codec::externals::bm_registry_info::BMRegistryInfoC;
use crate::codec::messages::bm_encoding::values_from_c;
use crate::codec::messages::touch::Touch;
use crate::engine::events::{Command, Sensor};
use crate::types::touch_state::TouchState;

use super::types::*;
use super::*;

pub(super) fn bytes_from_c(ptr: *const c_uchar, len: usize) -> Option<Vec<u8>> {
    if len == 0 {
        return Some(Vec::new());
    }
    if ptr.is_null() {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    Some(bytes.to_vec())
}

pub(super) fn info_from_c(
    ptr: *const BMRegistryInfoC,
) -> Option<crate::codec::externals::bm_registry_info::BMRegistryInfo> {
    if ptr.is_null() {
        return None;
    }
    unsafe { &*ptr }.to_rust()
}

pub(super) fn touches_from_c(ptr: *const TouchPointC, len: usize) -> Option<Vec<Touch>> {
    if len == 0 {
        return Some(Vec::new());
    }
    if ptr.is_null() {
        return None;
    }
    let items = unsafe { std::slice::from_raw_parts(ptr, len) };
    let mut touches = Vec::with_capacity(len);
    for t in items {
        touches.push(Touch {
            id: t.id,
            x: t.x,
            y: t.y,
            screen_width: t.screen_width,
            screen_height: t.screen_height,
            state: TouchState::from_value(t.state)?,
        });
    }
    Some(touches)
}

pub(super) fn sensor_from_c(v: i32) -> Option<Sensor> {
    Some(match v {
        0 => Sensor::Touch,
        1 => Sensor::Accel,
        2 => Sensor::Gyro,
        3 => Sensor::Orientation,
        _ => return None,
    })
}

pub(super) fn command_from_c(c: &CommandC) -> Option<Command> {
    Some(match CommandTagC::from_i32(c.tag)? {
        CommandTagC::Raw => Command::Raw {
            target: req_str(c.target)?,
            channel: c.channel,
            reliability: c.reliability,
            payload: bytes_from_c(c.payload_ptr, c.payload_len)?,
        },
        CommandTagC::Invoke => Command::Invoke {
            target: req_str(c.target)?,
            method: req_str(c.method)?,
            return_method: opt_str(c.return_method)?,
            params: values_from_c(c.params_ptr, c.params_len)?,
        },
        CommandTagC::Relay => Command::Relay {
            target: req_str(c.target)?,
            destination: info_from_c(c.info)?,
            method: req_str(c.method)?,
            return_method: opt_str(c.return_method)?,
            params: values_from_c(c.params_ptr, c.params_len)?,
        },
        CommandTagC::ApproveRegistration => Command::ApproveRegistration {
            device_id: req_str(c.device_id)?,
        },
        CommandTagC::DenyRegistration => Command::DenyRegistration {
            device_id: req_str(c.device_id)?,
        },
        CommandTagC::DropDevice => Command::DropDevice {
            device_id: req_str(c.device_id)?,
        },
        CommandTagC::Register => Command::Register {
            target: req_str(c.target)?,
            info: info_from_c(c.info)?,
            domain: opt_str(c.domain)?,
            return_method: opt_str(c.return_method)?,
        },
        CommandTagC::RequestHostList => Command::RequestHostList {
            target: req_str(c.target)?,
            return_method: opt_str(c.return_method)?,
        },
        CommandTagC::UpdateHostInfo => Command::UpdateHostInfo {
            target: req_str(c.target)?,
            info: info_from_c(c.info)?,
            return_method: opt_str(c.return_method)?,
        },
        CommandTagC::Unregister => Command::Unregister {
            target: req_str(c.target)?,
            return_method: opt_str(c.return_method)?,
        },
        CommandTagC::SetHostVisible => Command::SetHostVisible {
            target: req_str(c.target)?,
            visible: c.visible,
            notify_everyone: c.notify_everyone,
        },
        CommandTagC::ConnectToHost => Command::ConnectToHost {
            target: req_str(c.target)?,
            host: info_from_c(c.info)?,
            self_info: info_from_c(c.self_info)?,
        },
        CommandTagC::SendTouch => Command::SendTouch {
            target: req_str(c.target)?,
            touches: touches_from_c(c.touches_ptr, c.touches_len)?,
        },
        CommandTagC::SendAccel => Command::SendAccel {
            target: req_str(c.target)?,
            x: c.x,
            y: c.y,
            z: c.z,
        },
        CommandTagC::SendGyro => Command::SendGyro {
            target: req_str(c.target)?,
            x: c.x as f32,
            y: c.y as f32,
            z: c.z as f32,
        },
        CommandTagC::SendOrientation => Command::SendOrientation {
            target: req_str(c.target)?,
            x: c.x as f32,
            y: c.y as f32,
            z: c.z as f32,
            w: c.w as f32,
        },
        CommandTagC::SendDPad => Command::SendDPad {
            target: req_str(c.target)?,
            x: c.dpad_x,
            y: c.dpad_y,
        },
        CommandTagC::SendButton => Command::SendButton {
            target: req_str(c.target)?,
            handler: req_str(c.name)?,
            pressed: c.pressed,
        },
        CommandTagC::SendMenuEvent => Command::SendMenuEvent {
            target: req_str(c.target)?,
            event: req_str(c.name)?,
        },
        CommandTagC::SendKeyString => Command::SendKeyString {
            target: req_str(c.target)?,
            key: req_str(c.name)?,
        },
        CommandTagC::SendNavigation => Command::SendNavigation {
            target: req_str(c.target)?,
            nav: req_str(c.name)?,
        },
        CommandTagC::SetCapabilities => Command::SetCapabilities {
            target: req_str(c.target)?,
            gyroscope: c.gyroscope,
            orientation: c.orientation,
        },
        CommandTagC::ConfigureSensor => Command::ConfigureSensor {
            target: req_str(c.target)?,
            sensor: sensor_from_c(c.sensor)?,
            enabled: match c.enabled {
                v if v < 0 => None,
                0 => Some(false),
                _ => Some(true),
            },
            interval_ms: if c.interval_ms < 0 {
                None
            } else {
                Some(c.interval_ms)
            },
        },
        CommandTagC::SetReliability => Command::SetReliability {
            target: req_str(c.target)?,
            touch: c.touch_reliability,
            sensors: c.sensors_reliability,
        },
        CommandTagC::SetControlMode => Command::SetControlMode {
            target: req_str(c.target)?,
            mode: c.mode,
            text: opt_str(c.value)?,
        },
        CommandTagC::Vibrate => Command::Vibrate {
            target: req_str(c.target)?,
        },
        CommandTagC::Pause => Command::Pause {
            target: req_str(c.target)?,
        },
        CommandTagC::RequestControlScheme => Command::RequestControlScheme {
            target: req_str(c.target)?,
            width: c.width,
            height: c.height,
        },
        CommandTagC::SendControlScheme => Command::SendControlScheme {
            target: req_str(c.target)?,
            xml: bytes_from_c(c.payload_ptr, c.payload_len)?,
        },
        CommandTagC::ControlSchemeParsed => Command::ControlSchemeParsed {
            target: req_str(c.target)?,
        },
        CommandTagC::StoreCookie => Command::StoreCookie {
            target: req_str(c.target)?,
            name: req_str(c.name)?,
            value: req_str(c.value)?,
        },
        CommandTagC::RequestCookie => Command::RequestCookie {
            target: req_str(c.target)?,
            name: req_str(c.name)?,
        },
        CommandTagC::SendCookie => Command::SendCookie {
            target: req_str(c.target)?,
            name: req_str(c.name)?,
            value: req_str(c.value)?,
        },
    })
}
