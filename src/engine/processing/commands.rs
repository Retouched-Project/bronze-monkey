// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use super::Engine;
use crate::codec::externals::bm_reliability::BMReliability;
use crate::codec::messages::bm_encoding::Value;
use crate::codec::messages::bm_invoke::BMInvoke;
use crate::codec::object::Object;
use crate::controls::builder::SchemeBuilder;
use crate::controls::parser::BMApplicationSchemeParser;
use crate::engine::events::{Command, EmitError, Outgoing, ProcessOutput, Sensor};
use crate::engine::methods;
use crate::types::channel_type::ChannelType;
use crate::types::packet_type::PacketType;

impl Engine {
    /// Turns a command into what goes on the wire.
    ///
    /// A command naming a peer the engine does not have is refused. A
    /// departure takes the peer with it, so this covers a send that arrived
    /// after one as well as a name that was never right, and does not try to
    /// tell them apart.
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

        if let Some(target) = Self::target_of(&cmd) {
            if target.is_empty() {
                return Err(EmitError::EmptyTarget);
            }
            if self.state.registry.get(target).is_none() {
                return Err(EmitError::UnknownDevice {
                    device_id: target.to_string(),
                });
            }
        }

        let outgoings = match cmd {
            Command::Raw {
                target,
                channel,
                reliability,
                payload,
            } => vec![self.dispatch(target, channel, reliability, payload)],
            Command::SendObject {
                target,
                object,
                channel,
                reliability,
            } => {
                let channel = channel.unwrap_or_else(|| Self::default_channel_for_object(&object));
                let reliability = reliability.unwrap_or_else(|| self.reliability_for(channel));
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
            Command::DeclareTouch { enabled } => {
                self.declare_touch(enabled);
                Vec::new()
            }
            Command::TouchEvent { target, events } => {
                self.take_touch_events(&target, events, &mut out.next_send_ms)
            }
            Command::SendTouch { target, touches } => {
                let reliability = self.reliability_for(ChannelType::Touch.value());
                self.make_touch_set(&target, touches, reliability)
            }
            Command::SendAccel { target, x, y, z } => {
                let paced = self.sensor_due(Sensor::Accel);
                out.next_send_ms = paced.next_send_ms;
                if !paced.send {
                    Vec::new()
                } else {
                    let reliability = self.reliability_for(ChannelType::Acceleration.value());
                    self.make_accel(&target, x, y, z, reliability)
                }
            }
            Command::SendGyro { target, x, y, z } => {
                let paced = self.sensor_due(Sensor::Gyro);
                out.next_send_ms = paced.next_send_ms;
                if !paced.send {
                    Vec::new()
                } else {
                    let reliability = self.reliability_for(ChannelType::Gyro.value());
                    self.make_gyro(&target, x as f32, y as f32, z as f32, reliability)
                }
            }
            Command::SendOrientation { target, x, y, z, w } => {
                let paced = self.sensor_due(Sensor::Orientation);
                out.next_send_ms = paced.next_send_ms;
                if !paced.send {
                    Vec::new()
                } else {
                    let reliability = self.reliability_for(ChannelType::Orientation.value());
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
            } => {
                let unreliable = BMReliability::Unreliable.code();
                if !self.datagrams && (touch == unreliable || sensors == unreliable) {
                    log::warn!(
                        "asking '{target}' for unreliable input leaves it nowhere to send: \
                         this endpoint has no unreliable path to read one on"
                    );
                }
                self.make_set_reliability_for_touch(&target, touch, sensors)
            }
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
            Command::SendControlScheme { target, xml } => {
                self.send_scheme_document(&target, crate::controls::CONTROL_SCHEME_SET_ID, &xml)
            }
            Command::UpdateScheme { target, xml } => {
                self.send_scheme_document(&target, crate::controls::UPDATE_SCHEME_SET_ID, &xml)
            }
            Command::LoadScheme {
                index,
                xml,
                for_screen,
            } => {
                if let Err(e) = self.schemes.load(index, &xml, for_screen) {
                    return Err(EmitError::BadScheme(e));
                }
                // Every handler the scheme names becomes dispatchable, so a
                // button cannot arrive as silence for want of a registration.
                let handlers = self.schemes.button_handlers();
                self.register_button_handlers(handlers);
                Vec::new()
            }
            Command::AssignScheme { device, index } => {
                if device.is_empty() {
                    return Err(EmitError::EmptyTarget);
                }
                self.schemes.assign(&device, index);
                Vec::new()
            }
            Command::BeginScheme {
                index,
                width,
                height,
                orientation,
                touch_enabled,
                accelerometer_enabled,
                sample,
                for_screen,
            } => {
                self.schemes.begin(
                    index,
                    SchemeBuilder::new(
                        width,
                        height,
                        &orientation,
                        touch_enabled,
                        accelerometer_enabled,
                        &sample,
                    ),
                    for_screen,
                );
                Vec::new()
            }
            Command::AddImage {
                index,
                name,
                rect,
                artwork,
            } => {
                self.edit_scheme(index, |b| b.add_image(&name, rect, &artwork))?;
                Vec::new()
            }
            Command::AddButton {
                index,
                name,
                handler,
                rect,
                up,
                down,
            } => {
                self.edit_scheme(index, |b| b.add_button(&name, &handler, rect, &up, &down))?;
                Vec::new()
            }
            Command::AddDPad {
                index,
                name,
                handler,
                rect,
                states,
                deadzone,
                radial,
            } => {
                let refs: Vec<&[u8]> = states.iter().map(|s| s.as_slice()).collect();
                let count = refs.len();
                let states: [&[u8]; 9] = refs.as_slice().try_into().map_err(|_| {
                    EmitError::BadScheme(format!("a dpad needs nine states, got {count}"))
                })?;
                self.edit_scheme(index, |b| {
                    b.add_dpad(&name, &handler, rect, states, deadzone, radial)
                })?;
                Vec::new()
            }
            Command::AddText {
                index,
                name,
                rect,
                text,
                size,
                color,
            } => {
                self.edit_scheme(index, |b| b.add_text(&name, rect, &text, size, color))?;
                Vec::new()
            }
            Command::SetRect { index, name, rect } => {
                self.edit_scheme(index, |b| b.set_rect(&name, rect))?;
                Vec::new()
            }
            Command::SetHitRect { index, name, rect } => {
                self.edit_scheme(index, |b| b.set_hit_rect(&name, rect))?;
                Vec::new()
            }
            Command::SetObjectHidden {
                index,
                name,
                hidden,
            } => {
                self.edit_scheme(index, |b| b.set_hidden(&name, hidden))?;
                Vec::new()
            }
            Command::SetObjectPage { index, name, page } => {
                self.edit_scheme(index, |b| b.set_page(&name, page))?;
                Vec::new()
            }
            Command::ShowPage { index, page } => {
                self.edit_scheme(index, |b| {
                    b.show_page(page);
                    Ok(())
                })?;
                Vec::new()
            }
            Command::SetSamplingMode { index, name, mode } => {
                self.edit_scheme(index, |b| b.set_sampling_mode(&name, &mode))?;
                Vec::new()
            }
            Command::ClearHitRect { index, name } => {
                self.edit_scheme(index, |b| b.clear_hit_rect(&name))?;
                Vec::new()
            }
            Command::SetColor { index, name, color } => {
                self.edit_scheme(index, |b| b.set_color(&name, color))?;
                Vec::new()
            }
            Command::SetTextSize { index, name, size } => {
                self.edit_scheme(index, |b| b.set_text_size(&name, size))?;
                Vec::new()
            }
            Command::SetDeadzone {
                index,
                name,
                deadzone,
            } => {
                self.edit_scheme(index, |b| b.set_deadzone(&name, deadzone))?;
                Vec::new()
            }
            Command::SetRadial {
                index,
                name,
                radial,
            } => {
                self.edit_scheme(index, |b| b.set_radial(&name, radial))?;
                Vec::new()
            }
            Command::RemoveMenuOption { index, title } => {
                self.edit_scheme(index, |b| b.remove_menu_option(&title))?;
                Vec::new()
            }
            Command::SetObjectText { index, name, text } => {
                self.edit_scheme(index, |b| b.set_text(&name, &text))?;
                Vec::new()
            }
            Command::ReplaceArtwork {
                index,
                name,
                asset,
                artwork,
            } => {
                self.edit_scheme(index, |b| b.replace_artwork(&name, &asset, &artwork))?;
                Vec::new()
            }
            Command::RemoveObject { index, name } => {
                self.edit_scheme(index, |b| b.remove(&name))?;
                Vec::new()
            }
            Command::AddMenuOption {
                index,
                title,
                event,
                close_on_select,
                icon,
            } => {
                self.edit_scheme(index, |b| {
                    b.add_menu_option(&title, &event, close_on_select, icon);
                    Ok(())
                })?;
                Vec::new()
            }
            Command::SendSchemeUpdate { target, index } => {
                // Without an index this goes to whichever scheme the device is
                // already holding, since sending it an update to a different
                // one would merge a layout into a document it never received.
                let index = index
                    .or_else(|| self.schemes.index_for_device(&target))
                    .ok_or_else(|| {
                        EmitError::BadScheme(format!("no scheme is being served to '{target}'"))
                    })?;
                let xml = self
                    .schemes
                    .take_update(index)
                    .map_err(EmitError::BadScheme)?;
                // No parse on the way out: the handlers were registered as the
                // scheme was built, so there is nothing here left to learn.
                self.make_byte_chunks(&target, crate::controls::UPDATE_SCHEME_SET_ID, &xml)
            }
            Command::PeerReachable { device } => self.peer_reachable(device),
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

    /// Who a command is addressed to, for the commands addressed to anyone.
    ///
    /// Exhaustive by design. A command added without a line here does not
    /// compile.
    fn target_of(cmd: &Command) -> Option<&str> {
        match cmd {
            Command::ConfigureSensor { target, .. }
            | Command::ConnectToHost { target, .. }
            | Command::ControlSchemeParsed { target, .. }
            | Command::Invoke { target, .. }
            | Command::Pause { target, .. }
            | Command::Ping { target, .. }
            | Command::PromptTrialUpsell { target, .. }
            | Command::Raw { target, .. }
            | Command::Register { target, .. }
            | Command::Relay { target, .. }
            | Command::ReportConnectionFailed { target, .. }
            | Command::RequestControlScheme { target, .. }
            | Command::RequestCookie { target, .. }
            | Command::RequestHostList { target, .. }
            | Command::SendAccel { target, .. }
            | Command::SendButton { target, .. }
            | Command::SendControlScheme { target, .. }
            | Command::SendSchemeUpdate { target, .. }
            | Command::UpdateScheme { target, .. }
            | Command::SendCookie { target, .. }
            | Command::SendDPad { target, .. }
            | Command::SendGyro { target, .. }
            | Command::SendKeyString { target, .. }
            | Command::SendMenuEvent { target, .. }
            | Command::SendNavigation { target, .. }
            | Command::SendObject { target, .. }
            | Command::SendOrientation { target, .. }
            | Command::SendTouch { target, .. }
            | Command::SetCapabilities { target, .. }
            | Command::SetControlMode { target, .. }
            | Command::SetHostVisible { target, .. }
            | Command::SetReliability { target, .. }
            | Command::StoreCookie { target, .. }
            | Command::TouchEvent { target, .. }
            | Command::Unregister { target, .. }
            | Command::UpdateHostInfo { target, .. }
            | Command::UpdateWallet { target, .. }
            | Command::Vibrate { target, .. }
            | Command::WaitForNewHost { target, .. } => Some(target),

            // Nothing leaves for a named peer, so there is nobody to be wrong
            // about. `PeerReachable` names one but is how the engine hears of
            // it, so it cannot be asked to know it already.
            Command::AddButton { .. }
            | Command::AddDPad { .. }
            | Command::AddImage { .. }
            | Command::AddMenuOption { .. }
            | Command::AddText { .. }
            | Command::ApproveRegistration { .. }
            | Command::AssignScheme { .. }
            | Command::BeginScheme { .. }
            | Command::DeclareTouch { .. }
            | Command::DenyRegistration { .. }
            | Command::LoadScheme { .. }
            | Command::PeerGone { .. }
            | Command::PeerReachable { .. }
            | Command::RemoveObject { .. }
            | Command::ReplaceArtwork { .. }
            | Command::SetHitRect { .. }
            | Command::SetObjectHidden { .. }
            | Command::SetObjectPage { .. }
            | Command::SetObjectText { .. }
            | Command::SetRect { .. }
            | Command::ClearHitRect { .. }
            | Command::RemoveMenuOption { .. }
            | Command::SetColor { .. }
            | Command::SetDeadzone { .. }
            | Command::SetRadial { .. }
            | Command::SetSamplingMode { .. }
            | Command::SetTextSize { .. }
            | Command::ShowPage { .. } => None,
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

    /// Applies one change to a scheme the game is building.
    ///
    /// Every edit re-registers the handlers the library now names, so a button
    /// added mid game is dispatchable the moment it exists rather than once
    /// somebody remembers to declare it.
    fn edit_scheme(
        &mut self,
        index: u32,
        change: impl FnOnce(&mut SchemeBuilder) -> Result<(), String>,
    ) -> Result<(), EmitError> {
        let builder = self.schemes.edit(index).map_err(EmitError::BadScheme)?;
        change(builder).map_err(EmitError::BadScheme)?;
        let handlers = self.schemes.button_handlers();
        self.register_button_handlers(handlers);
        Ok(())
    }

    fn send_scheme_document(&mut self, target: &str, set_id: &str, xml: &[u8]) -> Vec<Outgoing> {
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
            Err(e) => log::warn!("{set_id} would not parse, sending it anyway: {e}"),
        }
        self.make_byte_chunks(target, set_id, xml)
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

    #[test]
    fn a_scheme_command_survives_the_encoding_the_bindings_use() {
        use crate::controls::builder::Rect;

        let sent = Command::AddDPad {
            index: 2,
            name: "pad".to_string(),
            handler: "onPad".to_string(),
            rect: Rect::new(20.0, 40.0, 160.0, 160.0),
            states: (0..9)
                .map(|i| serde_bytes::ByteBuf::from(vec![i as u8; 4]))
                .collect(),
            deadzone: 0.3,
            radial: true,
        };

        let bytes = rmp_serde::to_vec_named(&sent).expect("a command encodes");
        let back: Command = rmp_serde::from_slice(&bytes).expect("and reads back");

        match back {
            Command::AddDPad {
                index,
                name,
                rect,
                states,
                deadzone,
                radial,
                ..
            } => {
                assert_eq!(index, 2);
                assert_eq!(name, "pad");
                assert_eq!(rect, Rect::new(20.0, 40.0, 160.0, 160.0));
                assert_eq!(states.len(), 9);
                assert_eq!(states[3].as_slice(), &[3u8; 4]);
                assert_eq!(deadzone, 0.3);
                assert!(radial);
            }
            other => panic!("came back as something else: {other:?}"),
        }
    }

    #[test]
    fn a_scheme_command_without_a_screen_still_reads() {
        #[derive(serde::Serialize)]
        struct NoScreen {
            r#type: &'static str,
            index: u32,
            width: i32,
            height: i32,
            orientation: &'static str,
            touch_enabled: bool,
            accelerometer_enabled: bool,
            sample: &'static str,
        }

        let bytes = rmp_serde::to_vec_named(&NoScreen {
            r#type: "BeginScheme",
            index: 0,
            width: 480,
            height: 320,
            orientation: "landscape",
            touch_enabled: true,
            accelerometer_enabled: false,
            sample: "linear",
        })
        .expect("encodes");

        match rmp_serde::from_slice::<Command>(&bytes).expect("reads as a command") {
            Command::BeginScheme {
                width, for_screen, ..
            } => {
                assert_eq!(width, 480);
                assert!(for_screen.is_none());
            }
            other => panic!("came back as something else: {other:?}"),
        }
    }

    fn engine_with_peer(peer: &str) -> Engine {
        let mut eng = Engine::new();
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
        eng.push_registry_update(DeviceRecord::new(
            DeviceCore::new(
                "server".to_string(),
                "Registry".to_string(),
                DeviceType::Server,
            ),
            None,
        ));

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

    /// A departure takes the peer with it, so a send that arrives after one is
    /// addressed to nobody and is told so. A caller that was racing a
    /// disconnect can ignore this; a caller with a stale device id cannot
    /// afford to.
    #[test]
    fn a_send_to_a_departed_peer_is_refused() {
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
        assert_eq!(
            out.unwrap_err(),
            EmitError::UnknownDevice {
                device_id: "game1".to_string()
            }
        );
    }

    #[test]
    fn a_send_to_a_peer_that_was_never_here_is_refused() {
        let mut eng = engine_with_peer("game1");
        let out = eng.emit(
            Command::Vibrate {
                target: "stranger".to_string(),
            },
            None,
        );
        assert_eq!(
            out.unwrap_err(),
            EmitError::UnknownDevice {
                device_id: "stranger".to_string()
            }
        );
    }

    /// A controller is named to a game twice, once by the registry and once by
    /// the consumer that dialled it. The second naming must not cost the first
    /// one's registration, or reporting the dial fails would find nothing.
    #[test]
    fn reporting_a_peer_again_keeps_what_was_registered_about_it() {
        let mut eng = engine_with_peer("server");
        let listed = registry_info("phone", DeviceType::Android);
        eng.state.upsert_registry_info(listed);

        eng.emit(
            Command::PeerReachable {
                device: DeviceCore::new(
                    "phone".to_string(),
                    "Phone".to_string(),
                    DeviceType::Android,
                ),
            },
            None,
        )
        .expect("a peer we already had a listing for");

        assert!(
            eng.registry_info_of("phone").is_some(),
            "the listing has to survive being named again"
        );
    }

    /// Naming a peer is how the engine hears of it, so this is the one command
    /// that cannot be asked to name one it already knows.
    #[test]
    fn reporting_a_peer_is_not_held_to_knowing_it_already() {
        let mut eng = engine_with_peer("game1");
        eng.emit(
            Command::PeerReachable {
                device: DeviceCore::new(
                    "newcomer".to_string(),
                    "Newcomer".to_string(),
                    DeviceType::Android,
                ),
            },
            None,
        )
        .expect("a peer the engine has never heard of is the whole point");
        assert!(eng.registry().get("newcomer").is_some());
    }

    #[test]
    fn a_peer_that_comes_back_is_addressable_again() {
        let mut eng = engine_with_peer("game1");
        eng.peer_gone("game1");
        eng.push_registry_update(DeviceRecord::new(
            DeviceCore::new("game1".to_string(), "Game".to_string(), DeviceType::Unity),
            None,
        ));
        let out = eng
            .emit(
                Command::Vibrate {
                    target: "game1".to_string(),
                },
                None,
            )
            .expect("it is here again");
        assert!(!out.outgoings.is_empty());
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
