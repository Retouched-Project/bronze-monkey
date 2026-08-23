// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::controls::{AppResource, ContextMenuOption, ControlAsset, ControlScheme, DisplayObject};
use quick_xml::events::Event;
use quick_xml::reader::Reader;

pub struct BMApplicationSchemeParser {
    scheme: ControlScheme,
    sampling_mode: String,
}

impl Default for BMApplicationSchemeParser {
    fn default() -> Self {
        Self::new()
    }
}

impl BMApplicationSchemeParser {
    pub fn new() -> Self {
        Self {
            scheme: ControlScheme::default(),
            sampling_mode: "linear".to_string(),
        }
    }

    pub fn parse(&mut self, xml_data: &[u8]) -> Result<ControlScheme, String> {
        let mut reader = Reader::from_reader(xml_data);
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();
        let mut current_resource: Option<AppResource> = None;
        let mut current_display_object: Option<DisplayObject> = None;
        let mut in_data_element = false;
        let mut data_buffer = String::new();

        loop {
            let event = reader.read_event_into(&mut buf);
            let is_empty = matches!(&event, Ok(Event::Empty(_)));
            match event {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    match e.name().as_ref() {
                        b"BMApplicationScheme" => {
                            for attr in e.attributes() {
                                let attr = attr.map_err(|e| e.to_string())?;
                                let val = std::str::from_utf8(&attr.value).unwrap_or("");
                                match attr.key.as_ref() {
                                    b"accelerometerEnabled" => {
                                        self.scheme.accelerometer_enabled = self.is_yes(val)
                                    }
                                    b"orientation" => self.scheme.orientation = val.to_string(),
                                    b"touchEnabled" => self.scheme.touch_enabled = self.is_yes(val),
                                    b"version" => self.scheme.version = val.to_string(),
                                    b"width" => {
                                        if let Ok(v) = val.parse::<i32>() {
                                            self.scheme.width = v;
                                        }
                                    }
                                    b"height" => {
                                        if let Ok(v) = val.parse::<i32>() {
                                            self.scheme.height = v;
                                        }
                                    }
                                    b"sample" => self.sampling_mode = val.to_string(),
                                    _ => {}
                                }
                            }
                        }
                        b"Resource" => {
                            let mut res = AppResource::default();
                            for attr in e.attributes() {
                                let attr = attr.map_err(|e| e.to_string())?;
                                if attr.key.as_ref() == b"id" {
                                    let val = std::str::from_utf8(&attr.value).unwrap_or("0");
                                    if let Ok(v) = val.parse::<i32>() {
                                        res.id = v;
                                    } else {
                                        // self.scheme.debug_log.push(format!("Failed to parse Resource ID from: '{}'", val));
                                    }
                                }
                            }
                            current_resource = Some(res);
                        }
                        b"DisplayObject" => {
                            let mut obj = DisplayObject {
                                sampling_mode: self.sampling_mode.clone(),
                                deadzone: 0.25,
                                ..Default::default()
                            };
                            for attr in e.attributes() {
                                let attr = attr.map_err(|e| e.to_string())?;
                                let val = std::str::from_utf8(&attr.value).unwrap_or("");
                                match attr.key.as_ref() {
                                    b"functionHandler" => obj.function_handler = val.to_string(),
                                    b"height" => {
                                        if let Ok(v) = val.parse::<f32>() {
                                            obj.height = v;
                                        }
                                    }
                                    b"left" => {
                                        if let Ok(v) = val.parse::<f32>() {
                                            obj.left = v;
                                        }
                                    }
                                    b"top" => {
                                        if let Ok(v) = val.parse::<f32>() {
                                            obj.top = v;
                                        }
                                    }
                                    b"type" => obj.r#type = val.to_string(),
                                    b"width" => {
                                        if let Ok(v) = val.parse::<f32>() {
                                            obj.width = v;
                                        }
                                    }
                                    b"id" => {
                                        if let Ok(v) = val.parse::<i32>() {
                                            obj.id = v;
                                        }
                                    }
                                    b"hidden" => obj.hidden = self.is_yes(val),
                                    b"text" => obj.text = val.to_string(),
                                    b"textSize" => {
                                        if let Ok(v) = val.parse::<f32>() {
                                            obj.text_size = v;
                                        }
                                    }
                                    b"color" => {
                                        // 6-hex is opaque, 8-hex carries its own alpha
                                        let hex = val.trim_start_matches('#');
                                        let parsed = match hex.len() {
                                            6 => u32::from_str_radix(hex, 16)
                                                .ok()
                                                .map(|v| v | 0xFF00_0000),
                                            8 => u32::from_str_radix(hex, 16).ok(),
                                            _ => None,
                                        };
                                        if let Some(v) = parsed {
                                            obj.color = v as i32;
                                        }
                                    }
                                    b"sample" => obj.sampling_mode = val.to_string(),
                                    b"deadzone" => {
                                        // values above 1.0 are ignored, keeping the default
                                        if let Ok(v) = val.parse::<f32>()
                                            && v <= 1.0
                                        {
                                            obj.deadzone = v;
                                        }
                                    }
                                    b"radial" => obj.radial = self.is_yes(val),
                                    _ => {}
                                }
                            }
                            if is_empty {
                                self.scheme.display_objects.push(obj);
                            } else {
                                current_display_object = Some(obj);
                            }
                        }
                        b"Asset" => {
                            if let Some(obj) = &mut current_display_object {
                                let mut asset = ControlAsset::default();
                                for attr in e.attributes() {
                                    let attr = attr.map_err(|e| e.to_string())?;
                                    let val = std::str::from_utf8(&attr.value).unwrap_or("");
                                    match attr.key.as_ref() {
                                        b"name" => asset.name = val.to_string(),
                                        b"resourceRef" => {
                                            if let Ok(v) = val.parse::<i32>() {
                                                asset.resource_ref = v;
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                obj.assets.push(asset);
                            }
                        }
                        b"HitRect" => {
                            if let Some(obj) = &mut current_display_object {
                                obj.has_hit_rect = true;
                                for attr in e.attributes() {
                                    let attr = attr.map_err(|e| e.to_string())?;
                                    let val = std::str::from_utf8(&attr.value).unwrap_or("0");
                                    if let Ok(v) = val.parse::<f32>() {
                                        match attr.key.as_ref() {
                                            b"left" => obj.hit_left = v,
                                            b"top" => obj.hit_top = v,
                                            b"width" => obj.hit_width = v,
                                            b"height" => obj.hit_height = v,
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                        b"Option" => {
                            let mut opt = ContextMenuOption::default();
                            for attr in e.attributes() {
                                let attr = attr.map_err(|e| e.to_string())?;
                                let val = std::str::from_utf8(&attr.value).unwrap_or("");
                                match attr.key.as_ref() {
                                    b"title" => opt.title = val.to_string(),
                                    b"event" => opt.event = val.to_string(),
                                    b"icon" => {
                                        if let Ok(v) = val.parse::<i32>() {
                                            opt.icon_res_id = v;
                                        }
                                    }
                                    b"close" => opt.close_on_select = self.is_yes(val),
                                    _ => {}
                                }
                            }
                            self.scheme.options.push(opt);
                        }
                        b"data" => {
                            in_data_element = true;
                            data_buffer.clear();
                        }
                        _ => {}
                    }
                }
                Ok(Event::Text(e)) => {
                    if in_data_element && let Ok(text) = e.unescape() {
                        data_buffer.push_str(&text);
                    }
                }
                Ok(Event::CData(e)) => {
                    if in_data_element && let Ok(text) = std::str::from_utf8(&e) {
                        data_buffer.push_str(text);
                    }
                }
                Ok(Event::End(e)) => {
                    match e.name().as_ref() {
                        b"Resource" => {
                            if let Some(res) = current_resource.take() {
                                self.scheme.resources.push(res);
                            }
                        }
                        b"DisplayObject" => {
                            if let Some(obj) = current_display_object.take() {
                                self.scheme.display_objects.push(obj);
                            }
                        }
                        b"data" => {
                            in_data_element = false;
                            if let Some(res) = &mut current_resource {
                                let clean_text: String =
                                    data_buffer.chars().filter(|c| !c.is_whitespace()).collect();
                                match crate::base64_decode(&clean_text) {
                                    Ok(bytes) => {
                                        res.bitmap = bytes;
                                    }
                                    Err(_e) => {
                                        // self.scheme.debug_log.push(format!("Resource {}: base64 decode error: {:?}", res.id, e));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(e.to_string()),
                _ => {}
            }
            buf.clear();
        }

        Ok(self.scheme.clone())
    }

    fn is_yes(&self, val: &str) -> bool {
        val != "no"
    }
}

#[cfg(test)]
mod tests {
    use super::BMApplicationSchemeParser;
    use crate::controls::DisplayObject;

    fn parse_one(attrs: &str) -> DisplayObject {
        let xml = format!(
            "<BMApplicationScheme version=\"1.0\" width=\"480\" height=\"320\"><DisplayObject id=\"1\" {attrs}/></BMApplicationScheme>"
        );
        let mut parser = BMApplicationSchemeParser::new();
        let scheme = parser.parse(xml.as_bytes()).unwrap();
        scheme
            .display_objects
            .into_iter()
            .next()
            .expect("one display object")
    }

    #[test]
    fn color_6hex_is_opaque() {
        assert_eq!(parse_one("color=\"ff0000\"").color, 0xFFFF_0000u32 as i32);
    }

    #[test]
    fn color_8hex_keeps_alpha() {
        assert_eq!(parse_one("color=\"80ff0000\"").color, 0x80FF_0000u32 as i32);
    }

    #[test]
    fn deadzone_defaults_to_quarter() {
        assert_eq!(parse_one("").deadzone, 0.25);
    }

    #[test]
    fn deadzone_above_one_keeps_default() {
        assert_eq!(parse_one("deadzone=\"2.0\"").deadzone, 0.25);
    }

    #[test]
    fn deadzone_valid_applied_including_zero() {
        assert_eq!(parse_one("deadzone=\"0.3\"").deadzone, 0.3);
        assert_eq!(parse_one("deadzone=\"0\"").deadzone, 0.0);
    }
}
