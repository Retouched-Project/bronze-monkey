// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::controls::ControlScheme;

pub fn apply_update(base: &mut ControlScheme, update: ControlScheme) {
    // Exhaustive destructure: if scheme.proto gains a field, this fails to compile and
    // forces a deliberate merge decision for it.
    let ControlScheme {
        version: _,
        orientation: _,
        touch_enabled: _,
        accelerometer_enabled: _,
        width: _,
        height: _,
        sample: _,
        resources,
        display_objects,
        options,
        // Produced here, never carried in: a scheme from the parser has none.
        changed_resources: _,
    } = update;

    // Resources: overlay by id. Keep base ids absent from the update; replace in
    // place when present; append new ids.
    let mut changed = Vec::with_capacity(resources.len());
    for res in resources {
        changed.push(res.id);
        if let Some(existing) = base.resources.iter_mut().find(|r| r.id == res.id) {
            *existing = res;
        } else {
            base.resources.push(res);
        }
    }

    // The ids the update carried. Everything else in base.resources is byte for
    // byte what the consumer already has.
    base.changed_resources = changed;

    // Display objects: replace with the update's set (absent objects are dropped).
    base.display_objects = display_objects;

    // Options: replace unconditionally (an empty update clears them; the game owns its menu).
    base.options = options;

    // Scalars: kept from base.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::{AppResource, ContextMenuOption, DisplayObject};

    fn res(id: i32, byte: u8) -> AppResource {
        AppResource {
            id,
            bitmap: vec![byte],
            ..Default::default()
        }
    }

    fn obj(id: i32, ty: &str) -> DisplayObject {
        DisplayObject {
            id,
            r#type: ty.to_string(),
            ..Default::default()
        }
    }

    fn opt(title: &str) -> ContextMenuOption {
        ContextMenuOption {
            title: title.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn resources_overlay_by_id_keeping_absent() {
        let mut base = ControlScheme {
            resources: vec![res(1, 0xA), res(2, 0xB), res(3, 0xC)],
            ..Default::default()
        };
        let update = ControlScheme {
            resources: vec![res(2, 0xFF), res(4, 0xD)],
            display_objects: vec![obj(1, "button")],
            ..Default::default()
        };
        apply_update(&mut base, update);
        // 1 and 3 kept, 2 replaced in place, 4 appended.
        assert_eq!(
            base.resources,
            vec![res(1, 0xA), res(2, 0xFF), res(3, 0xC), res(4, 0xD)]
        );
    }

    #[test]
    fn layout_only_update_keeps_resources_and_scalars() {
        let mut base = ControlScheme {
            version: "0.1".into(),
            orientation: "landscape".into(),
            width: 480,
            height: 320,
            touch_enabled: true,
            accelerometer_enabled: true,
            sample: "nearest".into(),
            resources: vec![res(1, 0xA), res(2, 0xB)],
            display_objects: vec![obj(10, "button"), obj(11, "image")],
            options: vec![],
            changed_resources: vec![],
        };
        // A partial update: parser output has default scalars, no resources, layout only.
        let update = ControlScheme {
            display_objects: vec![obj(10, "button")],
            ..Default::default()
        };
        apply_update(&mut base, update);
        assert_eq!(base.resources, vec![res(1, 0xA), res(2, 0xB)]);
        assert_eq!(base.orientation, "landscape");
        assert_eq!(base.width, 480);
        assert_eq!(base.height, 320);
        assert!(base.touch_enabled);
        assert!(base.accelerometer_enabled);
        assert_eq!(base.sample, "nearest");
        // image #11 dropped.
        assert_eq!(base.display_objects, vec![obj(10, "button")]);
    }

    #[test]
    fn changed_resources_reports_only_the_ids_the_update_carried() {
        let mut base = ControlScheme {
            resources: vec![res(1, 0xA), res(2, 0xB), res(3, 0xC)],
            ..Default::default()
        };
        let update = ControlScheme {
            // 2 replaced in place, 4 appended, 1 and 3 untouched.
            resources: vec![res(2, 0xFF), res(4, 0xD)],
            display_objects: vec![obj(1, "button")],
            ..Default::default()
        };
        apply_update(&mut base, update);
        assert_eq!(base.changed_resources, vec![2, 4]);
    }

    #[test]
    fn layout_only_update_reports_nothing_changed() {
        let mut base = ControlScheme {
            resources: vec![res(1, 0xA)],
            // A previous update left its ids behind; this one must clear them.
            changed_resources: vec![1],
            ..Default::default()
        };
        let update = ControlScheme {
            display_objects: vec![obj(10, "button")],
            ..Default::default()
        };
        apply_update(&mut base, update);
        assert!(base.changed_resources.is_empty());
        assert_eq!(base.resources, vec![res(1, 0xA)]);
    }

    #[test]
    fn changed_resources_is_never_carried_in_from_the_update() {
        let mut base = ControlScheme::default();
        let update = ControlScheme {
            // The parser never sets this, but a hand built scheme could.
            changed_resources: vec![99],
            display_objects: vec![obj(1, "button")],
            ..Default::default()
        };
        apply_update(&mut base, update);
        assert!(base.changed_resources.is_empty());
    }

    #[test]
    fn display_objects_replaced_dropping_absent() {
        let mut base = ControlScheme {
            display_objects: vec![obj(1, "button"), obj(2, "dpad"), obj(3, "image")],
            ..Default::default()
        };
        let update = ControlScheme {
            display_objects: vec![obj(2, "dpad")],
            ..Default::default()
        };
        apply_update(&mut base, update);
        assert_eq!(base.display_objects, vec![obj(2, "dpad")]);
    }

    #[test]
    fn options_replaced_unconditionally_including_clear() {
        let mut base = ControlScheme {
            options: vec![opt("Quit"), opt("Help")],
            ..Default::default()
        };
        // Empty options in the update clear them (the game is authoritative).
        apply_update(&mut base, ControlScheme::default());
        assert!(base.options.is_empty());

        let mut base2 = ControlScheme {
            options: vec![opt("Quit")],
            ..Default::default()
        };
        apply_update(
            &mut base2,
            ControlScheme {
                options: vec![opt("Resume"), opt("Settings")],
                ..Default::default()
            },
        );
        assert_eq!(base2.options, vec![opt("Resume"), opt("Settings")]);
    }

    #[test]
    fn scalars_never_overlaid_from_update() {
        let mut base = ControlScheme {
            version: "0.1".into(),
            orientation: "landscape".into(),
            width: 480,
            height: 320,
            touch_enabled: true,
            accelerometer_enabled: false,
            ..Default::default()
        };
        // Even a header-bearing update must not change scalars.
        let update = ControlScheme {
            version: "9.9".into(),
            orientation: "portrait".into(),
            width: 1,
            height: 2,
            touch_enabled: false,
            accelerometer_enabled: true,
            display_objects: vec![obj(1, "button")],
            ..Default::default()
        };
        apply_update(&mut base, update);
        assert_eq!(base.version, "0.1");
        assert_eq!(base.orientation, "landscape");
        assert_eq!(base.width, 480);
        assert_eq!(base.height, 320);
        assert!(base.touch_enabled);
        assert!(!base.accelerometer_enabled);
    }
}
