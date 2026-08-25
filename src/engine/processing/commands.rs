// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use super::Engine;
use crate::codec::messages::bm_encoding::Value;
use crate::codec::messages::bm_invoke::BMInvoke;
use crate::codec::object::Object;
use crate::controls::parser::BMApplicationSchemeParser;
use crate::engine::events::{Command, EmitError, Outgoing, ProcessOutput, Sensor};
use crate::engine::methods;
use crate::types::channel_type::ChannelType;
use crate::types::packet_type::PacketType;

impl Engine {
    /// Turns a command into what goes on the wire.
    ///
    /// An error means the call itself was wrong, and never that the session
    /// cannot use it right now: a send to a peer that has since left comes
    /// back as no outgoings at all, since a race between input and a
    /// departure is ordinary protocol life.
    ///
    /// The answer is shaped like any other, because a command can start
    /// something the clock has to finish: whatever it schedules is named here
    /// rather than waiting for the next packet to arrive.
    ///
    /// `now_ms` is the caller's clock, on whatever monotonic scale it keeps.
    /// It is what lets a paced command know whether its turn has come, and a
    /// caller that has no clock passes nothing and is never held back.
    pub fn emit(&mut self, cmd: Command, now_ms: Option<u64>) -> Result<ProcessOutput, EmitError> {
        let mut out = ProcessOutput::new();
        let now = now_ms.map(|now_ms| self.set_clock(now_ms));

        let outgoings = match cmd {
            Command::Raw {
                target,
                channel,
                reliability,
                payload,
            } => {
                if target.is_empty() {
                    return Err(EmitError::EmptyTarget);
                }
                vec![self.dispatch(target, channel, reliability, payload)]
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
                    Err(e) => return Err(EmitError::Encode(e.to_string())),
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
            Command::PeerGone { device_id } => self.peer_gone(&device_id),
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
            Command::ConnectToHost { target, host_id } => {
                let Some(host) = self.registry_info_of(&host_id) else {
                    return Err(EmitError::UnknownDevice { device_id: host_id });
                };
                let Some(self_info) = self.state.local_info.clone() else {
                    return Err(EmitError::NotRegistered);
                };
                self.reset_game_session();
                self.make_device_connect_requested(&target, host, self_info)
            }
            Command::ReportConnectionFailed {
                target,
                controller_id,
            } => {
                let Some(controller) = self.registry_info_of(&controller_id) else {
                    return Err(EmitError::UnknownDevice {
                        device_id: controller_id,
                    });
                };
                self.make_connection_failed(&target, controller)
            }
            Command::TouchEvent { target, events } => {
                self.take_touch_events(&target, events, &mut out.next_send_ms)
            }
            Command::SendTouch { target, touches } => {
                let reliability = self.reliability_for(&target, ChannelType::Touch.value());
                self.make_touch_set(&target, touches, reliability)
            }
            Command::SendAccel { target, x, y, z } => {
                let paced = self.sensor_due(Sensor::Accel);
                out.next_send_ms = paced.next_send_ms;
                if !paced.send {
                    Vec::new()
                } else {
                    let reliability =
                        self.reliability_for(&target, ChannelType::Acceleration.value());
                    self.make_accel(&target, x, y, z, reliability)
                }
            }
            Command::SendGyro { target, x, y, z } => {
                let paced = self.sensor_due(Sensor::Gyro);
                out.next_send_ms = paced.next_send_ms;
                if !paced.send {
                    Vec::new()
                } else {
                    let reliability = self.reliability_for(&target, ChannelType::Gyro.value());
                    self.make_gyro(&target, x as f32, y as f32, z as f32, reliability)
                }
            }
            Command::SendOrientation { target, x, y, z, w } => {
                let paced = self.sensor_due(Sensor::Orientation);
                out.next_send_ms = paced.next_send_ms;
                if !paced.send {
                    Vec::new()
                } else {
                    let reliability =
                        self.reliability_for(&target, ChannelType::Orientation.value());
                    self.make_orientation(
                        &target,
                        x as f32,
                        y as f32,
                        z as f32,
                        w as f32,
                        reliability,
                    )
                }
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
        };
        out.outgoings.extend(outgoings);
        if let Some(now) = now {
            self.run_due(now, &mut out);
        }
        out.next_time_ms = self.next_deadline();
        Ok(out)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::externals::bm_packet::BMPacket;
    use crate::codec::externals::bm_reliability::BMReliability;
    use crate::devices::device_core::DeviceCore;
    use crate::engine::device_registry::DeviceRecord;
    use crate::engine::events::Via;
    use crate::engine::protocol::deserialize_message;
    use crate::link::framing::Framer;
    use crate::types::device_type::DeviceType;

    fn engine_with_peer(peer: &str) -> Engine {
        let mut eng = Engine::default();
        eng.init_local_device(DeviceCore::new(
            "local".to_string(),
            "Local".to_string(),
            DeviceType::Android,
        ));
        eng.push_registry_update(DeviceRecord::new(
            DeviceCore::new(peer.to_string(), "Game".to_string(), DeviceType::Flash),
            None,
        ));
        eng
    }

    /// A peer that has told us a port, and a caller that can write to it.
    fn engine_with_datagrams(peer: &str) -> Engine {
        let mut eng = engine_with_peer(peer);
        eng.configure(crate::config::EngineConfig {
            datagrams: true,
            ..Default::default()
        })
        .expect("nothing else is configured");
        let mut core = DeviceCore::new(peer.to_string(), "Game".to_string(), DeviceType::Flash);
        core.address = Some(crate::devices::bm_address::BMAddress::new(
            "10.0.0.2".to_string(),
            9080,
            9081,
        ));
        eng.push_registry_update(DeviceRecord::new(core, None));
        eng
    }

    #[test]
    fn an_outgoing_carries_a_message_with_no_length_in_front() {
        let mut eng = engine_with_peer("game1");
        let out = eng
            .emit(
                Command::SendDPad {
                    target: "game1".to_string(),
                    x: 1,
                    y: 2,
                },
                None,
            )
            .unwrap()
            .outgoings;
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].via, Via::Stream);

        // It arrives ready to write, so a stream reads it back whole.
        let mut framer = Framer::new();
        let back = framer
            .feed(&out[0].payload)
            .expect("payload should be framed");
        assert_eq!(back.len(), 1);

        let mut pkt = BMPacket::default();
        deserialize_message(&back[0], &mut pkt).expect("the frame should hold a message");
        assert_eq!(pkt.device_id, "local");
    }

    #[test]
    fn a_stream_bound_message_is_not_a_bare_one() {
        let mut eng = engine_with_peer("game1");
        let out = eng
            .emit(
                Command::SendDPad {
                    target: "game1".to_string(),
                    x: 3,
                    y: 4,
                },
                None,
            )
            .unwrap()
            .outgoings;

        // The length in front is what makes it writable, and it is part of the
        // payload, so the payload does not read as a bare message.
        let mut bare = BMPacket::default();
        assert!(deserialize_message(&out[0].payload, &mut bare).is_err());
    }

    #[test]
    fn a_datagram_is_taken_when_reliability_asks_and_a_path_exists() {
        // Unreliable traffic alone is not enough: without an unreliable path
        // the caller could not write a bare message anywhere.
        let mut eng = engine_with_peer("game1");
        let sensors = eng
            .emit(
                Command::SendAccel {
                    target: "game1".to_string(),
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                None,
            )
            .unwrap()
            .outgoings;
        assert_eq!(sensors[0].reliability, BMReliability::Unreliable.code());
        assert_eq!(sensors[0].via, Via::Stream, "no datagram path was declared");

        let mut eng = engine_with_datagrams("game1");
        let sensors = eng
            .emit(
                Command::SendAccel {
                    target: "game1".to_string(),
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                None,
            )
            .unwrap()
            .outgoings;
        assert_eq!(
            sensors[0].via,
            Via::Datagram {
                address: "10.0.0.2".to_string(),
                port: 9080
            },
            "the engine says where, so nothing else has to work it out"
        );
        // A datagram carries the message as it is.
        let mut pkt = BMPacket::default();
        deserialize_message(&sensors[0].payload, &mut pkt).expect("a datagram is a bare message");

        let control = eng
            .emit(
                Command::SendButton {
                    target: "game1".to_string(),
                    handler: "a".to_string(),
                    pressed: true,
                },
                None,
            )
            .unwrap()
            .outgoings;
        assert_eq!(control[0].via, Via::Stream, "control traffic goes reliably");
    }

    /// Reliability is the game's to set, through setReliabilityForTouch, and a
    /// game that wants its input reliably says so.
    #[test]
    fn a_peer_with_no_port_still_gets_what_reliability_asked_for() {
        let mut eng = engine_with_peer("game1");
        eng.configure(crate::config::EngineConfig {
            datagrams: true,
            ..Default::default()
        })
        .expect("nothing else is configured");

        let sensors = eng
            .emit(
                Command::SendAccel {
                    target: "game1".to_string(),
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                None,
            )
            .unwrap()
            .outgoings;
        assert_eq!(
            sensors[0].via,
            Via::Datagram {
                address: String::new(),
                port: 0
            },
            "an empty endpoint fails loudly where a stream would hide it"
        );
    }

    /// A host list entry and our own registration are both things the engine
    /// already holds, so asking to be introduced needs neither passed back.
    #[test]
    fn an_introduction_is_built_from_what_the_engine_already_knows() {
        let mut eng = engine_with_peer("game1");
        let listed = registry_info("game1", DeviceType::Unity);
        eng.state.upsert_registry_info(listed);

        // Nothing registered yet, so there is no way to say who we are.
        let refused = eng.emit(
            Command::ConnectToHost {
                target: "server".to_string(),
                host_id: "game1".to_string(),
            },
            None,
        );
        assert_eq!(
            refused.unwrap_err(),
            EmitError::NotRegistered,
            "it cannot invent a registration, and says so"
        );

        eng.push_registry_update(DeviceRecord::new(
            DeviceCore::new(
                "server".to_string(),
                "Registry".to_string(),
                DeviceType::Server,
            ),
            None,
        ));
        eng.emit(
            Command::Register {
                target: "server".to_string(),
                info: registry_info("local", DeviceType::Android),
                domain: None,
                return_method: None,
            },
            None,
        )
        .unwrap();

        let out = eng
            .emit(
                Command::ConnectToHost {
                    target: "server".to_string(),
                    host_id: "game1".to_string(),
                },
                None,
            )
            .unwrap()
            .outgoings;
        assert_eq!(out.len(), 1, "the introduction goes to the registry");
        assert_eq!(out[0].target_device_id, "server");
    }

    #[test]
    fn an_unknown_host_is_refused_rather_than_guessed() {
        let mut eng = engine_with_peer("game1");
        eng.emit(
            Command::Register {
                target: "game1".to_string(),
                info: registry_info("local", DeviceType::Android),
                domain: None,
                return_method: None,
            },
            None,
        )
        .unwrap();
        let out = eng.emit(
            Command::ConnectToHost {
                target: "game1".to_string(),
                host_id: "never-heard-of-it".to_string(),
            },
            None,
        );
        assert_eq!(
            out.unwrap_err(),
            EmitError::UnknownDevice {
                device_id: "never-heard-of-it".to_string()
            }
        );
    }

    /// A send racing a departure is not a mistake: the wire would have
    /// dropped it, so it comes back as nothing to send rather than an error.
    #[test]
    fn a_send_to_a_departed_peer_is_dropped_not_refused() {
        let mut eng = engine_with_peer("game1");
        eng.peer_gone("game1");
        let out = eng.emit(
            Command::SendAccel {
                target: "game1".to_string(),
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            None,
        );
        assert!(out.unwrap().outgoings.is_empty());
    }

    #[test]
    fn a_command_with_no_target_is_refused() {
        let mut eng = engine_with_peer("game1");
        let out = eng.emit(
            Command::Raw {
                target: String::new(),
                channel: 3,
                reliability: 2,
                payload: vec![1, 2, 3],
            },
            None,
        );
        assert_eq!(out.unwrap_err(), EmitError::EmptyTarget);
    }

    fn registry_info(
        id: &str,
        kind: DeviceType,
    ) -> crate::codec::externals::bm_registry_info::BMRegistryInfo {
        let mut device = DeviceCore::new(id.to_string(), id.to_string(), kind);
        let address =
            crate::devices::bm_address::BMAddress::new("10.0.0.2".to_string(), 9080, 9081);
        device.address = Some(address.clone());
        crate::codec::externals::bm_registry_info::BMRegistryInfo {
            slot_id: 0,
            app_id: "app".to_string(),
            current_players: None,
            max_players: None,
            device,
            device_address: address,
        }
    }
}
