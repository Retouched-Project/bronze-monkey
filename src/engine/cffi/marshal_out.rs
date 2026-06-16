// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::bm_registry_info::{
    BMRegistryInfoC, bm_registry_info_set_addr_inner, bm_registry_info_set_app_id_inner,
    bm_registry_info_set_device_id_inner, bm_registry_info_set_device_name_inner,
};
use crate::codec::messages::bm_encoding::Value;
use crate::codec::messages::bm_invoke::BMInvoke;
use crate::codec::messages::bm_parameter::VecOutput;
use crate::codec::object::Object;
use crate::engine::events::{Event, Outgoing, ProcessOutput};
use crate::engine::registry::DeviceRecord;

use super::types::*;
use super::*;

pub(super) fn encode_invoke_message(
    method: &str,
    return_method: Option<&str>,
    params: Vec<Value>,
) -> Vec<u8> {
    let inv = BMInvoke {
        id: 0,
        method: method.to_string(),
        return_method: return_method.map(|s| s.to_string()),
        params,
    };
    let mut out = VecOutput::default();
    let _ = Object::BMInvoke(inv).encode_with_marker(&mut out);
    out.buf
}

pub(super) fn set_event_registry(
    out: &mut EventC,
    infos: Vec<crate::codec::externals::bm_registry_info::BMRegistryInfo>,
) {
    let regs: Vec<BMRegistryInfoC> = infos.into_iter().map(registry_info_to_c).collect();
    let len = regs.len();
    let mut boxed = regs.into_boxed_slice();
    out.registry_ptr = boxed.as_mut_ptr();
    out.registry_len = len;
    std::mem::forget(boxed);
}

pub(super) fn outgoing_to_c(o: Outgoing) -> OutgoingC {
    let mut out = OutgoingC::default();
    out.channel = o.channel;
    out.reliability = o.reliability;
    outgoing_set_payload_inner(&mut out, o.payload.as_ptr(), o.payload.len());
    set_string_field(o.target_device_id, |p| {
        outgoing_set_target_device_id_inner(&mut out, p)
    });
    out
}

pub(super) fn outgoings_to_c(outgoings: Vec<Outgoing>) -> OutgoingListC {
    let converted: Vec<OutgoingC> = outgoings.into_iter().map(outgoing_to_c).collect();
    let len = converted.len();
    let mut boxed = converted.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    OutgoingListC { ptr, len }
}

pub(super) fn fill_record_fields(out: &mut EventC, record: &DeviceRecord) {
    set_string_field(record.core.device_id.clone(), |p| {
        event_set_device_id_inner(out, p)
    });
    set_string_field(record.core.device_name.clone(), |p| {
        event_set_device_name_inner(out, p)
    });
    out.device_type_code = record.core.device_type.code();
    out.class_id = record.class_id.map(|v| v as i32).unwrap_or(-1);
    if let Some(addr) = &record.core.address {
        set_string_field(addr.address.clone(), |p| event_set_addr_inner(out, p));
        out.has_address = true;
        out.addr_unreliable_port = addr.unreliable_port;
        out.addr_reliable_port = addr.reliable_port;
    }
}

pub(super) fn event_to_c(event: Event) -> EventC {
    let mut out = EventC::default();
    match event {
        Event::Handshake { current, minimum } => {
            out.tag = EventTagC::Handshake;
            out.handshake_current = current;
            out.handshake_minimum = minimum;
        }
        Event::PeerSeen { record } => {
            out.tag = EventTagC::PeerSeen;
            fill_record_fields(&mut out, &record);
        }
        Event::PeerConnected { record } => {
            out.tag = EventTagC::PeerConnected;
            fill_record_fields(&mut out, &record);
        }
        Event::ConnectionFailed { device_id } => {
            out.tag = EventTagC::ConnectionFailed;
            set_string_field(device_id, |p| event_set_device_id_inner(&mut out, p));
        }
        Event::Touch { sender, touches } => {
            out.tag = EventTagC::Touch;
            set_string_field(sender, |p| event_set_sender_inner(&mut out, p));
            let points: Vec<TouchPointC> = touches
                .iter()
                .map(|t| TouchPointC {
                    id: t.id,
                    x: t.x,
                    y: t.y,
                    screen_width: t.screen_width,
                    screen_height: t.screen_height,
                    state: t.state.value(),
                })
                .collect();
            let len = points.len();
            let mut boxed = points.into_boxed_slice();
            out.touches_ptr = boxed.as_mut_ptr();
            out.touches_len = len;
            std::mem::forget(boxed);
        }
        Event::Accel { sender, x, y, z } => {
            out.tag = EventTagC::Accel;
            set_string_field(sender, |p| event_set_sender_inner(&mut out, p));
            out.sensor_x = x;
            out.sensor_y = y;
            out.sensor_z = z;
        }
        Event::Gyro { sender, x, y, z } => {
            out.tag = EventTagC::Gyro;
            set_string_field(sender, |p| event_set_sender_inner(&mut out, p));
            out.sensor_x = x as f64;
            out.sensor_y = y as f64;
            out.sensor_z = z as f64;
        }
        Event::Orientation { sender, x, y, z, w } => {
            out.tag = EventTagC::Orientation;
            set_string_field(sender, |p| event_set_sender_inner(&mut out, p));
            out.sensor_x = x as f64;
            out.sensor_y = y as f64;
            out.sensor_z = z as f64;
            out.sensor_w = w as f64;
        }
        Event::DPad { sender, x, y } => {
            out.tag = EventTagC::DPad;
            set_string_field(sender, |p| event_set_sender_inner(&mut out, p));
            out.dpad_x = x;
            out.dpad_y = y;
        }
        Event::Button {
            sender,
            handler,
            pressed,
        } => {
            out.tag = EventTagC::Button;
            set_string_field(sender, |p| event_set_sender_inner(&mut out, p));
            set_string_field(handler, |p| event_set_name_inner(&mut out, p));
            out.pressed = pressed;
        }
        Event::MenuEvent { sender, event } => {
            out.tag = EventTagC::MenuEvent;
            set_string_field(sender, |p| event_set_sender_inner(&mut out, p));
            set_string_field(event, |p| event_set_name_inner(&mut out, p));
        }
        Event::KeyString { sender, key } => {
            out.tag = EventTagC::KeyString;
            set_string_field(sender, |p| event_set_sender_inner(&mut out, p));
            set_string_field(key, |p| event_set_name_inner(&mut out, p));
        }
        Event::Navigation { sender, nav } => {
            out.tag = EventTagC::Navigation;
            set_string_field(sender, |p| event_set_sender_inner(&mut out, p));
            set_string_field(nav, |p| event_set_name_inner(&mut out, p));
        }
        Event::Capabilities {
            sender,
            gyroscope,
            orientation,
        } => {
            out.tag = EventTagC::Capabilities;
            set_string_field(sender, |p| event_set_sender_inner(&mut out, p));
            out.cap_gyroscope = gyroscope;
            out.cap_orientation = orientation;
        }
        Event::Vibrate { sender } => {
            out.tag = EventTagC::Vibrate;
            set_string_field(sender, |p| event_set_sender_inner(&mut out, p));
        }
        Event::Pause { sender } => {
            out.tag = EventTagC::Pause;
            set_string_field(sender, |p| event_set_sender_inner(&mut out, p));
        }
        Event::ControlSchemeRequested {
            sender,
            width,
            height,
            requester,
        } => {
            out.tag = EventTagC::ControlSchemeRequested;
            set_string_field(sender, |p| event_set_sender_inner(&mut out, p));
            set_string_field(requester, |p| event_set_device_id_inner(&mut out, p));
            out.scheme_width = width;
            out.scheme_height = height;
        }
        Event::ControlSchemeParsed { sender, device_id } => {
            out.tag = EventTagC::ControlSchemeParsed;
            set_string_field(sender, |p| event_set_sender_inner(&mut out, p));
            set_string_field(device_id, |p| event_set_device_id_inner(&mut out, p));
        }
        Event::CookieRequested { sender, name } => {
            out.tag = EventTagC::CookieRequested;
            set_string_field(sender, |p| event_set_sender_inner(&mut out, p));
            set_string_field(name, |p| event_set_name_inner(&mut out, p));
        }
        Event::CookieStored {
            sender,
            name,
            value,
        } => {
            out.tag = EventTagC::CookieStored;
            set_string_field(sender, |p| event_set_sender_inner(&mut out, p));
            set_string_field(name, |p| event_set_name_inner(&mut out, p));
            set_string_field(value, |p| event_set_value_inner(&mut out, p));
        }
        Event::Cookie {
            sender,
            name,
            value,
        } => {
            out.tag = EventTagC::Cookie;
            set_string_field(sender, |p| event_set_sender_inner(&mut out, p));
            set_string_field(name, |p| event_set_name_inner(&mut out, p));
            set_string_field(value, |p| event_set_value_inner(&mut out, p));
        }
        Event::PeerRegistered {
            info,
            domain,
            success,
        } => {
            out.tag = EventTagC::PeerRegistered;
            out.registry_success = if success { 1 } else { 0 };
            if let Some(d) = domain {
                set_string_field(d, |p| event_set_domain_inner(&mut out, p));
            }
            set_event_registry(&mut out, vec![info]);
        }
        Event::RegistrationResult { success } => {
            out.tag = EventTagC::RegistrationResult;
            out.registry_success = if success { 1 } else { 0 };
        }
        Event::SlotAssigned { info } => {
            out.tag = EventTagC::SlotAssigned;
            set_event_registry(&mut out, vec![info]);
        }
        Event::HostConnected { info } => {
            out.tag = EventTagC::HostConnected;
            set_event_registry(&mut out, vec![info]);
        }
        Event::HostUpdated { info } => {
            out.tag = EventTagC::HostUpdated;
            set_event_registry(&mut out, vec![info]);
        }
        Event::HostDisconnected { info } => {
            out.tag = EventTagC::HostDisconnected;
            set_event_registry(&mut out, vec![info]);
        }
        Event::HostList { infos } => {
            out.tag = EventTagC::HostList;
            set_event_registry(&mut out, infos);
        }
        Event::DeviceConnectRequested { info } => {
            out.tag = EventTagC::DeviceConnectRequested;
            set_event_registry(&mut out, vec![info]);
        }
        Event::DeviceKilled { device_id } => {
            out.tag = EventTagC::DeviceKilled;
            set_string_field(device_id, |p| event_set_device_id_inner(&mut out, p));
        }
        Event::Invoke {
            sender,
            method,
            return_method,
            params,
        } => {
            out.tag = EventTagC::Invoke;
            if let Some(s) = sender {
                set_string_field(s, |p| event_set_sender_inner(&mut out, p));
            }
            let msg = encode_invoke_message(&method, return_method.as_deref(), params);
            set_string_field(method, |p| event_set_invoke_method_inner(&mut out, p));
            if let Some(rm) = return_method {
                set_string_field(rm, |p| event_set_invoke_return_method_inner(&mut out, p));
            }
            event_set_payload_inner(&mut out, msg.as_ptr(), msg.len());
        }
        Event::ChunkProgress {
            device_id,
            set_id,
            current,
            total,
        } => {
            out.tag = EventTagC::ChunkProgress;
            set_string_field(device_id, |p| event_set_device_id_inner(&mut out, p));
            set_string_field(set_id, |p| event_set_chunk_set_id_inner(&mut out, p));
            out.chunk_current = current;
            out.chunk_total = total;
        }
        Event::ChunkComplete {
            device_id,
            set_id,
            blob,
        } => {
            out.tag = EventTagC::ChunkComplete;
            set_string_field(device_id, |p| event_set_device_id_inner(&mut out, p));
            set_string_field(set_id, |p| event_set_chunk_set_id_inner(&mut out, p));
            event_set_payload_inner(&mut out, blob.as_ptr(), blob.len());
        }
        Event::ControlScheme { device_id, scheme } => {
            out.tag = EventTagC::ControlScheme;
            set_string_field(device_id, |p| event_set_device_id_inner(&mut out, p));
            event_set_payload_inner(&mut out, scheme.as_ptr(), scheme.len());
        }
        Event::ControlConfig(cfg) => {
            out.tag = EventTagC::ControlConfig;
            out.control_touch_enabled = cfg
                .touch_enabled
                .map(|v| if v { 1 } else { 0 })
                .unwrap_or(-1);
            out.control_accel_enabled = cfg
                .accel_enabled
                .map(|v| if v { 1 } else { 0 })
                .unwrap_or(-1);
            out.control_gyro_enabled = cfg
                .gyro_enabled
                .map(|v| if v { 1 } else { 0 })
                .unwrap_or(-1);
            out.control_orientation_enabled = cfg
                .orientation_enabled
                .map(|v| if v { 1 } else { 0 })
                .unwrap_or(-1);
            out.control_touch_interval_ms = cfg.touch_interval_ms.unwrap_or(-1);
            out.control_accel_interval_ms = cfg.accel_interval_ms.unwrap_or(-1);
            out.control_gyro_interval_ms = cfg.gyro_interval_ms.unwrap_or(-1);
            out.control_orientation_interval_ms = cfg.orientation_interval_ms.unwrap_or(-1);
            out.control_touch_reliability = cfg.touch_reliability.unwrap_or(-1);
            out.control_reliability = cfg.control_reliability.unwrap_or(-1);
            out.control_mode = cfg.control_mode.unwrap_or(-1);
            if let Some(p) = cfg.portal_id {
                set_string_field(p, |ptr| event_set_control_portal_id_inner(&mut out, ptr));
            }
            if let Some(r) = cfg.return_app_id {
                set_string_field(r, |ptr| {
                    event_set_control_return_app_id_inner(&mut out, ptr)
                });
            }
        }
    }
    out
}

pub(super) fn events_to_c(events: Vec<Event>) -> EventListC {
    let converted: Vec<EventC> = events.into_iter().map(event_to_c).collect();
    let len = converted.len();
    let mut boxed = converted.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    EventListC { ptr, len }
}

pub(super) fn process_output_to_c(out: ProcessOutput) -> ProcessOutputC {
    ProcessOutputC {
        events: events_to_c(out.events),
        outgoings: outgoings_to_c(out.outgoings),
    }
}

pub(super) fn registry_info_to_c(
    src: crate::codec::externals::bm_registry_info::BMRegistryInfo,
) -> BMRegistryInfoC {
    let mut out = BMRegistryInfoC::default();
    out.slot_id = src.slot_id as i32;
    out.current_players = src.current_players.unwrap_or(0) as i32;
    out.max_players = src.max_players.unwrap_or(0) as i32;
    out.device_type_code = src.device.device_type.code();
    set_string_field(src.app_id, |p| {
        bm_registry_info_set_app_id_inner(&mut out, p)
    });
    set_string_field(src.device.device_id, |p| {
        bm_registry_info_set_device_id_inner(&mut out, p)
    });
    set_string_field(src.device.device_name, |p| {
        bm_registry_info_set_device_name_inner(&mut out, p)
    });
    let addr = src.device_address;
    set_string_field(addr.address, |p| {
        bm_registry_info_set_addr_inner(&mut out, p)
    });
    out.addr_unreliable_port = addr.unreliable_port;
    out.addr_reliable_port = addr.reliable_port;
    out
}
