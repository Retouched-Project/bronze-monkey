// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

//! The writer against many control schemes seen on the wire.

use bronze_monkey::controls::parser::BMApplicationSchemeParser;
use bronze_monkey::controls::writer::{write_full, write_update};
use bronze_monkey::controls::{AppResource, ControlScheme, DisplayObject};
use std::path::PathBuf;

fn fixtures() -> Vec<(String, Vec<u8>)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schemes");
    let mut found: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension()? != "xml" {
                return None;
            }
            let name = path.file_name()?.to_string_lossy().into_owned();
            Some((name, std::fs::read(&path).ok()?))
        })
        .collect();
    found.sort();
    found
}

fn parse(xml: &[u8]) -> ControlScheme {
    BMApplicationSchemeParser::new()
        .parse(xml)
        .expect("fixture parses")
}

#[test]
fn every_fixture_survives_a_round_trip() {
    let fixtures = fixtures();
    // An empty directory would otherwise let this pass while testing nothing.
    assert!(
        fixtures.len() >= 46,
        "found only {} fixtures",
        fixtures.len()
    );

    let mut failed = Vec::new();
    for (name, xml) in &fixtures {
        let once = parse(xml);
        let twice = parse(write_full(&once).as_bytes());
        if once != twice {
            failed.push(format!("{name}: {}", first_difference(&once, &twice)));
        }
    }
    assert!(
        failed.is_empty(),
        "{} of {} fixtures changed:\n  {}",
        failed.len(),
        fixtures.len(),
        failed.join("\n  ")
    );
}

#[test]
fn the_fixtures_cover_what_we_think_they_do() {
    let schemes: Vec<_> = fixtures().iter().map(|(_, xml)| parse(xml)).collect();
    let objects = || schemes.iter().flat_map(|s| &s.display_objects);

    assert!(objects().any(|o| o.r#type == "text" && !o.text.is_empty()));
    assert!(objects().any(|o| o.r#type == "dpad"));
    assert!(objects().any(|o| o.has_hit_rect));
    assert!(objects().any(|o| !o.name.is_empty()));
    assert!(objects().any(|o| o.function_handler.is_empty()));
    assert!(schemes.iter().any(|s| !s.options.is_empty()));
    assert!(
        schemes
            .iter()
            .any(|s| s.options.iter().any(|o| !o.close_on_select))
    );
    assert!(schemes.iter().any(|s| s.accelerometer_enabled));
    assert!(schemes.iter().any(|s| !s.accelerometer_enabled));
    assert!(schemes.iter().any(|s| s.orientation == "portrait"));
    assert!(schemes.iter().any(|s| s.orientation == "landscape"));
    assert!(schemes.iter().any(|s| s.sample == "nearest"));
    assert!(
        schemes
            .iter()
            .any(|s| s.resources.iter().any(|r| r.id == 0))
    );
}

#[test]
fn an_update_carries_only_the_resources_marked_changed() {
    let mut scheme = parse(
        br#"<BMApplicationScheme width="480" height="320">
              <Resources>
                <Resource id="1" type="image"><data>AAECAw==</data></Resource>
                <Resource id="2" type="image"><data>BAUGBw==</data></Resource>
              </Resources>
              <Layout><DisplayObject id="1" type="button"/></Layout>
            </BMApplicationScheme>"#,
    );
    scheme.changed_resources = vec![2];

    let update = parse(write_update(&scheme).as_bytes());
    assert_eq!(update.resources.len(), 1);
    assert_eq!(update.resources[0].id, 2);
    assert_eq!(update.resources[0].bitmap, vec![4, 5, 6, 7]);
    // The layout is never filtered, only the resources are.
    assert_eq!(update.display_objects.len(), 1);

    assert_eq!(parse(write_full(&scheme).as_bytes()).resources.len(), 2);
}

#[test]
fn markup_in_a_label_is_escaped() {
    let scheme = ControlScheme {
        version: "0.1".into(),
        orientation: "landscape".into(),
        width: 480,
        height: 320,
        sample: "linear".into(),
        display_objects: vec![DisplayObject {
            id: 1,
            r#type: "text".into(),
            name: "a<b".into(),
            text: r#"Rock & Roll "quoted" <b>"#.into(),
            function_handler: "a>b".into(),
            deadzone: 0.25,
            sampling_mode: "linear".into(),
            ..Default::default()
        }],
        ..Default::default()
    };

    let xml = write_full(&scheme);
    assert!(
        !xml.contains("Rock & Roll"),
        "raw ampersand reached the wire"
    );

    let back = parse(xml.as_bytes());
    assert_eq!(back.display_objects, scheme.display_objects);
}

#[test]
fn an_object_sampling_differently_from_its_scheme_keeps_it() {
    let scheme = ControlScheme {
        sample: "nearest".into(),
        width: 480,
        height: 320,
        display_objects: vec![
            DisplayObject {
                id: 1,
                r#type: "button".into(),
                sampling_mode: "linear".into(),
                deadzone: 0.25,
                ..Default::default()
            },
            DisplayObject {
                id: 2,
                r#type: "button".into(),
                sampling_mode: "nearest".into(),
                deadzone: 0.25,
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let xml = write_full(&scheme);
    assert_eq!(xml.matches("sample=").count(), 2, "{xml}");
    assert_eq!(parse(xml.as_bytes()), scheme);
}

#[test]
fn a_scheme_built_from_nothing_round_trips() {
    let scheme = ControlScheme {
        resources: vec![AppResource {
            id: 0,
            bitmap: vec![1, 2, 3],
            ..Default::default()
        }],
        display_objects: vec![DisplayObject {
            id: 1,
            r#type: "dpad".into(),
            deadzone: 0.4,
            radial: true,
            sampling_mode: "linear".into(),
            ..Default::default()
        }],
        sample: "linear".into(),
        ..Default::default()
    };
    assert_eq!(parse(write_full(&scheme).as_bytes()), scheme);
}

/// Points at the first field that differs, since comparing two schemes that
/// each hold hundreds of objects is otherwise unreadable.
fn first_difference(a: &ControlScheme, b: &ControlScheme) -> String {
    macro_rules! scalar {
        ($($field:ident),*) => {$(
            if a.$field != b.$field {
                return format!("{} {:?} became {:?}", stringify!($field), a.$field, b.$field);
            }
        )*};
    }
    scalar!(
        version,
        orientation,
        touch_enabled,
        accelerometer_enabled,
        width,
        height,
        sample,
        options,
        changed_resources
    );

    if a.resources.len() != b.resources.len() {
        return format!(
            "{} resources became {}",
            a.resources.len(),
            b.resources.len()
        );
    }
    for (x, y) in a.resources.iter().zip(&b.resources) {
        if x != y {
            return format!("resource {} changed", x.id);
        }
    }
    if a.display_objects.len() != b.display_objects.len() {
        return format!(
            "{} display objects became {}",
            a.display_objects.len(),
            b.display_objects.len()
        );
    }
    for (x, y) in a.display_objects.iter().zip(&b.display_objects) {
        if x != y {
            return format!(
                "object {} ({}) changed:\n    {x:?}\n    {y:?}",
                x.id, x.r#type
            );
        }
    }
    "no field differs, but the schemes compare unequal".into()
}
