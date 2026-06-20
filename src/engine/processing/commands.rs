// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use super::Engine;
use crate::codec::messages::bm_encoding::Value;
use crate::codec::messages::bm_invoke::BMInvoke;
use crate::codec::object::Object;
use crate::controls::parser::BMApplicationSchemeParser;
use crate::engine::events::{Command, Outgoing, Sensor};
use crate::engine::methods;
use crate::types::channel_type::ChannelType;
use crate::types::packet_type::PacketType;

impl Engine {
    pub fn emit(&mut self, cmd: Command) -> Vec<Outgoing> {
        match cmd {
            Command::Raw {
                target,
                channel,
                reliability,
                payload,
            } => {
                if target.is_empty() {
                    log::warn!("emit Raw: target device id is empty");
                    return Vec::new();
                }
                vec![Outgoing {
                    target_device_id: target,
                    channel,
                    reliability,
                    payload,
                }]
            }
            Command::SendObject {
                target,
                object,
                channel,
                reliability,
            } => {
                let channel = channel.unwrap_or_else(|| Self::default_channel_for_object(&object));
                let reliability =
                    reliability.unwrap_or_else(|| self.reliability_for(&target, channel));
                let msg = match self.build_object_bytes(object) {
                    Ok(m) => m,
                    Err(e) => {
                        log::error!("emit SendObject: encode failed: {e}");
                        return Vec::new();
                    }
                };
                self.make_packet(
                    &target,
                    channel,
                    Some(reliability),
                    PacketType::Data,
                    Some(msg),
                )
            }
            Command::Invoke {
                target,
                method,
                return_method,
                params,
            } => self.make_message_invoke(&target, &method, return_method.as_deref(), params),
            Command::Relay {
                target,
                destination,
                method,
                return_method,
                params,
            } => {
                let inner = BMInvoke {
                    id: 0,
                    method,
                    return_method,
                    params,
                };
                self.make_registry_relay(&target, destination, inner)
            }
            Command::ApproveRegistration { device_id } => self.approve_registration(&device_id),
            Command::DenyRegistration { device_id } => self.deny_registration(&device_id),
            Command::DropDevice { device_id } => self.drop_device(&device_id),
            Command::Register {
                target,
                info,
                domain,
                return_method,
            } => self.make_registry_register(&target, info, domain, return_method.as_deref()),
            Command::RequestHostList {
                target,
                return_method,
            } => self.make_registry_list(&target, return_method.as_deref()),
            Command::UpdateHostInfo {
                target,
                info,
                return_method,
            } => self.make_message_invoke(
                &target,
                methods::REGISTRY_UPDATE,
                Some(Self::return_method_or(
                    return_method.as_deref(),
                    methods::DEFAULT_RETURN_UPDATE,
                )),
                vec![Value::Object(Object::BMRegistryInfo(info))],
            ),
            Command::Unregister {
                target,
                return_method,
            } => {
                let device_id = self.local_device_id();
                self.make_message_invoke(
                    &target,
                    methods::REGISTRY_REMOVE,
                    Some(Self::return_method_or(
                        return_method.as_deref(),
                        methods::DEFAULT_RETURN_REMOVE,
                    )),
                    vec![Value::String(device_id)],
                )
            }
            Command::SetHostVisible {
                target,
                visible,
                notify_everyone,
            } => self.make_message_invoke(
                &target,
                methods::REGISTRY_SET_VISIBLE,
                None,
                vec![Value::Bool(visible), Value::Bool(notify_everyone)],
            ),
            Command::ConnectToHost {
                target,
                host,
                self_info,
            } => self.make_device_connect_requested(&target, host, self_info),
            Command::SendTouch { target, touches } => {
                let reliability = self.reliability_for(&target, ChannelType::Touch.value());
                self.make_touch_set(&target, touches, reliability)
            }
            Command::SendAccel { target, x, y, z } => {
                let reliability = self.reliability_for(&target, ChannelType::Acceleration.value());
                self.make_accel(&target, x, y, z, reliability)
            }
            Command::SendGyro { target, x, y, z } => {
                let reliability = self.reliability_for(&target, ChannelType::Gyro.value());
                self.make_gyro(&target, x as f32, y as f32, z as f32, reliability)
            }
            Command::SendOrientation { target, x, y, z, w } => {
                let reliability = self.reliability_for(&target, ChannelType::Orientation.value());
                self.make_orientation(&target, x as f32, y as f32, z as f32, w as f32, reliability)
            }
            Command::SendDPad { target, x, y } => self.make_dpad_update(&target, x, y),
            Command::SendButton {
                target,
                handler,
                pressed,
            } => self.make_button_invoke(&target, &handler, pressed),
            Command::SendMenuEvent { target, event } => self.make_message_invoke(
                &target,
                methods::MENU_EVENT,
                None,
                vec![Value::String(event)],
            ),
            Command::SendKeyString { target, key } => self.make_message_invoke(
                &target,
                methods::ON_KEY_STRING,
                None,
                vec![Value::String(key)],
            ),
            Command::SendNavigation { target, nav } => self.make_message_invoke(
                &target,
                methods::ON_NAVIGATION_STRING,
                None,
                vec![Value::String(nav)],
            ),
            Command::SetCapabilities {
                target,
                gyroscope,
                orientation,
            } => {
                let mask = (gyroscope as u64) | ((orientation as u64) << 1);
                self.make_set_capabilities(&target, mask)
            }
            Command::ConfigureSensor {
                target,
                sensor,
                enabled,
                interval_ms,
            } => self.configure_sensor(&target, sensor, enabled, interval_ms),
            Command::SetReliability {
                target,
                touch,
                sensors,
            } => self.make_set_reliability_for_touch(&target, touch, sensors),
            Command::SetControlMode { target, mode, text } => {
                self.make_set_control_mode(&target, mode, text.as_deref())
            }
            Command::Vibrate { target } => self.make_vibrate(&target),
            Command::Pause { target } => {
                self.make_message_invoke(&target, methods::BM_PAUSE, None, vec![])
            }
            Command::Ping { target } => self.make_ping_packet(&target),
            Command::RequestControlScheme {
                target,
                width,
                height,
            } => {
                let requester = self.local_device_id();
                self.make_request_xml(&target, width, height, &requester)
            }
            Command::SendControlScheme { target, xml } => self.send_control_scheme(&target, &xml),
            Command::ControlSchemeParsed { target } => {
                let device_id = self.local_device_id();
                self.make_on_control_scheme_parsed(&target, &device_id)
            }
            Command::StoreCookie {
                target,
                name,
                value,
            } => self.make_set_cookie(&target, &name, &value),
            Command::RequestCookie { target, name } => self.make_get_cookie(&target, &name),
            Command::SendCookie {
                target,
                name,
                value,
            } => self.make_message_invoke(
                &target,
                methods::GOT_COOKIE,
                None,
                vec![Value::String(name), Value::String(value)],
            ),
            Command::UpdateWallet { target } => self.make_update_wallet(&target),
            Command::PromptTrialUpsell { target } => self.make_prompt_trial_upsell(&target),
            Command::WaitForNewHost {
                target,
                host_device_id,
            } => self.make_wait_for_new_host(&target, &host_device_id),
        }
    }

    fn configure_sensor(
        &mut self,
        target: &str,
        sensor: Sensor,
        enabled: Option<bool>,
        interval_ms: Option<i32>,
    ) -> Vec<Outgoing> {
        let interval_s = interval_ms.map(|ms| ms as f64 / 1000.0);
        let mut out = Vec::new();
        match sensor {
            Sensor::Accel => {
                if enabled.is_some() || interval_s.is_some() {
                    out.extend(self.make_enable_accelerometer(
                        target,
                        enabled.unwrap_or(true),
                        interval_s,
                    ));
                }
            }
            Sensor::Touch => {
                if let Some(enabled) = enabled {
                    out.extend(self.make_enable_touch(target, enabled));
                }
                if let Some(s) = interval_s {
                    out.extend(self.make_set_touch_interval(target, s));
                }
            }
            Sensor::Gyro => {
                if let Some(enabled) = enabled {
                    out.extend(self.make_enable_gyro(target, enabled));
                }
                if let Some(s) = interval_s {
                    out.extend(self.make_set_gyro_interval(target, s));
                }
            }
            Sensor::Orientation => {
                if let Some(enabled) = enabled {
                    out.extend(self.make_enable_orientation(target, enabled));
                }
                if let Some(s) = interval_s {
                    out.extend(self.make_set_orientation_interval(target, s));
                }
            }
        }
        out
    }

    fn send_control_scheme(&mut self, target: &str, xml: &[u8]) -> Vec<Outgoing> {
        let mut parser = BMApplicationSchemeParser::new();
        match parser.parse(xml) {
            Ok(scheme) => {
                let handlers: Vec<String> = scheme
                    .display_objects
                    .iter()
                    .map(|o| o.function_handler.clone())
                    .filter(|h| !h.is_empty())
                    .collect();
                self.register_button_handlers(handlers);
            }
            Err(e) => log::warn!("control scheme parse failed, sending anyway: {e}"),
        }
        self.make_byte_chunks(target, crate::controls::CONTROL_SCHEME_SET_ID, xml)
    }

    fn default_channel_for_object(object: &Object) -> i32 {
        match object {
            Object::TouchSet(_) => ChannelType::Touch.value(),
            Object::Acceleration(_) => ChannelType::Acceleration.value(),
            Object::BMGyro(_) => ChannelType::Gyro.value(),
            Object::Orientation(_) => ChannelType::Orientation.value(),
            Object::DPadUpdate(_) => ChannelType::DPad.value(),
            Object::BMByteChunk(_) => ChannelType::Bytes.value(),
            Object::StringLiteral(_) => ChannelType::String.value(),
            _ => ChannelType::Message.value(),
        }
    }
}
