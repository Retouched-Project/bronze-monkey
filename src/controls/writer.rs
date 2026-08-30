// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::base64_encode;
use crate::controls::parser::{DEFAULT_DEADZONE, DEFAULT_SAMPLING_MODE};
use crate::controls::{AppResource, ControlScheme, DisplayObject};
use quick_xml::Writer;
use quick_xml::events::{BytesCData, BytesDecl, BytesStart, Event};

/// A whole scheme, every resource included. Answers `RequestXML`.
pub fn write_full(scheme: &ControlScheme) -> String {
    write(scheme, ResourceSet::All)
}

/// An update. Carries the layout and the menu, but only the resources the
/// scheme says changed, which is the asymmetry the update form exists for.
pub fn write_update(scheme: &ControlScheme) -> String {
    write(scheme, ResourceSet::Changed)
}

enum ResourceSet {
    All,
    Changed,
}

fn write(scheme: &ControlScheme, resources: ResourceSet) -> String {
    let mut w = Writer::new(Vec::new());
    let _ = w.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)));

    let mut root = BytesStart::new("BMApplicationScheme");
    root.push_attribute(("version", scheme.version.as_str()));
    root.push_attribute(("orientation", scheme.orientation.as_str()));
    root.push_attribute(("touchEnabled", yes_no(scheme.touch_enabled)));
    root.push_attribute(("width", scheme.width.to_string().as_str()));
    root.push_attribute(("height", scheme.height.to_string().as_str()));
    root.push_attribute(("sample", sampling(&scheme.sample)));
    root.push_attribute(("accelerometerEnabled", yes_no(scheme.accelerometer_enabled)));
    let _ = w.write_event(Event::Start(root));

    // Ahead of Resources and Layout, where every observed document puts it.
    if !scheme.options.is_empty() {
        let _ = w.write_event(Event::Start(BytesStart::new("Menu")));
        for option in &scheme.options {
            let mut e = BytesStart::new("Option");
            e.push_attribute(("icon", option.icon_res_id.to_string().as_str()));
            e.push_attribute(("title", option.title.as_str()));
            e.push_attribute(("event", option.event.as_str()));
            e.push_attribute(("close", yes_no(option.close_on_select)));
            let _ = w.write_event(Event::Empty(e));
        }
        let _ = w.write_event(Event::End(BytesStart::new("Menu").to_end()));
    }

    let _ = w.write_event(Event::Start(BytesStart::new("Resources")));
    for res in &scheme.resources {
        let carried = match resources {
            ResourceSet::All => true,
            ResourceSet::Changed => scheme.changed_resources.contains(&res.id),
        };
        if carried {
            write_resource(&mut w, res);
        }
    }
    let _ = w.write_event(Event::End(BytesStart::new("Resources").to_end()));

    let _ = w.write_event(Event::Start(BytesStart::new("Layout")));
    for obj in &scheme.display_objects {
        write_object(&mut w, obj, &scheme.sample);
    }
    let _ = w.write_event(Event::End(BytesStart::new("Layout").to_end()));

    let _ = w.write_event(Event::End(BytesStart::new("BMApplicationScheme").to_end()));
    String::from_utf8(w.into_inner()).unwrap_or_default()
}

fn write_resource(w: &mut Writer<Vec<u8>>, res: &AppResource) {
    let mut e = BytesStart::new("Resource");
    e.push_attribute(("id", res.id.to_string().as_str()));
    if !res.r#type.is_empty() {
        e.push_attribute(("type", res.r#type.as_str()));
    }
    let _ = w.write_event(Event::Start(e));
    let _ = w.write_event(Event::Start(BytesStart::new("data")));
    let _ = w.write_event(Event::CData(BytesCData::new(base64_encode(&res.bitmap))));
    let _ = w.write_event(Event::End(BytesStart::new("data").to_end()));
    let _ = w.write_event(Event::End(BytesStart::new("Resource").to_end()));
}

fn write_object(w: &mut Writer<Vec<u8>>, obj: &DisplayObject, scheme_sample: &str) {
    let mut e = BytesStart::new("DisplayObject");
    e.push_attribute(("id", obj.id.to_string().as_str()));
    if !obj.name.is_empty() {
        e.push_attribute(("name", obj.name.as_str()));
    }
    e.push_attribute(("type", obj.r#type.as_str()));
    e.push_attribute(("hidden", yes_no(obj.hidden)));
    e.push_attribute(("top", num(obj.top).as_str()));
    e.push_attribute(("left", num(obj.left).as_str()));
    e.push_attribute(("width", num(obj.width).as_str()));
    e.push_attribute(("height", num(obj.height).as_str()));

    // Attributes an object only carries when it departs from the default it
    // would otherwise inherit, so a document does not grow a copy of the
    // scheme's own settings on every object.
    if sampling(&obj.sampling_mode) != sampling(scheme_sample) {
        e.push_attribute(("sample", sampling(&obj.sampling_mode)));
    }
    if obj.deadzone != DEFAULT_DEADZONE {
        e.push_attribute(("deadzone", num(obj.deadzone).as_str()));
    }
    if obj.radial {
        e.push_attribute(("radial", "yes"));
    }
    if !obj.function_handler.is_empty() {
        e.push_attribute(("functionHandler", obj.function_handler.as_str()));
    }
    if !obj.text.is_empty() {
        e.push_attribute(("text", obj.text.as_str()));
    }
    if obj.text_size != 0.0 {
        e.push_attribute(("textSize", num(obj.text_size).as_str()));
    }
    if obj.color != 0 {
        e.push_attribute(("color", color(obj.color).as_str()));
    }

    if obj.assets.is_empty() && !obj.has_hit_rect {
        let _ = w.write_event(Event::Empty(e));
        return;
    }
    let _ = w.write_event(Event::Start(e));
    for asset in &obj.assets {
        let mut a = BytesStart::new("Asset");
        a.push_attribute(("name", asset.name.as_str()));
        a.push_attribute(("resourceRef", asset.resource_ref.to_string().as_str()));
        let _ = w.write_event(Event::Empty(a));
    }
    if obj.has_hit_rect {
        let mut h = BytesStart::new("HitRect");
        h.push_attribute(("top", num(obj.hit_top).as_str()));
        h.push_attribute(("left", num(obj.hit_left).as_str()));
        h.push_attribute(("width", num(obj.hit_width).as_str()));
        h.push_attribute(("height", num(obj.hit_height).as_str()));
        let _ = w.write_event(Event::Empty(h));
    }
    let _ = w.write_event(Event::End(BytesStart::new("DisplayObject").to_end()));
}

fn yes_no(v: bool) -> &'static str {
    if v { "yes" } else { "no" }
}

fn sampling(mode: &str) -> &str {
    if mode.is_empty() {
        DEFAULT_SAMPLING_MODE
    } else {
        mode
    }
}

fn num(v: f32) -> String {
    v.to_string()
}

/// Opaque colours go out as the bare six hex the endpoints write, and only a
/// real alpha forces the eight hex form.
fn color(color: i32) -> String {
    let c = color as u32;
    if c >> 24 == 0xFF {
        format!("{:06x}", c & 0x00FF_FFFF)
    } else {
        format!("{:08x}", c)
    }
}
