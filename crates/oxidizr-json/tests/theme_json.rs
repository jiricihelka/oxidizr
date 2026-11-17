//! `theme.json` byte parity (P2-V1 oracle probes, 2026-11-17). Expected
//! bytes are the reference witnesses verbatim
//! (`/tmp/nix-shell.uni100/p2/v1/out3/styles-theme.json`,
//! `out6/order-theme.json` — `export --format theme`): `ThemeUtils.toJson`
//! declaration-order keys, `NON_EMPTY` theme fields / style arrays, per-field
//! `NON_NULL` style fields, no trailing newline.

use oxidizr_core::views::{ElementStyle, RelationshipStyle, Theme};
use oxidizr_json::{theme_from_slice, theme_write};

fn style_props(pairs: &[(&str, &str)]) -> oxidizr_core::properties::Properties {
    let mut props = oxidizr_core::properties::Properties::default();
    for (k, v) in pairs {
        props.insert((*k).to_string(), (*v).to_string());
    }
    props
}

/// `out6/order-theme.json` — `properties` sits right after the parent-field
/// keys (`tag`, `colorScheme`), before the declared `width`; the relationship
/// `position` entered the reference as `150` and came back `100` (setter
/// clamp on READ — the input carried the clamped bean).
const OUT6: &str = r##"{
  "elements" : [ {
    "tag" : "Prop",
    "properties" : {
      "pk" : "pv"
    },
    "width" : 10,
    "background" : "#111111",
    "opacity" : 50
  } ],
  "relationships" : [ {
    "tag" : "Rel",
    "properties" : {
      "rk" : "rv"
    },
    "thickness" : 2,
    "position" : 100,
    "opacity" : 50
  } ]
}"##;

#[test]
fn theme_write_matches_the_out6_witness_bytes() {
    let theme = Theme {
        elements: vec![ElementStyle {
            tag: "Prop".into(),
            width: Some(10),
            background: Some("#111111".into()),
            opacity: Some(50),
            properties: style_props(&[("pk", "pv")]),
            ..Default::default()
        }],
        relationships: vec![RelationshipStyle {
            tag: "Rel".into(),
            thickness: Some(2),
            position: Some(100),
            opacity: Some(50),
            properties: style_props(&[("rk", "rv")]),
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_eq!(String::from_utf8(theme_write(&theme)).unwrap(), OUT6);
}

#[test]
fn theme_round_trip_is_stable_and_clamps_on_read() {
    // The witness reads back byte-identical (read+write symmetry), and
    // out-of-range numbers normalize through the same setters as the
    // workspace reader (`strokeWidth 0 -> 1`).
    let parsed = theme_from_slice(OUT6.as_bytes()).expect("theme reads");
    assert_eq!(String::from_utf8(theme_write(&parsed)).unwrap(), OUT6);
    assert_eq!(parsed.relationships[0].position, Some(100));

    let raw = r#"{ "elements": [ { "tag": "A", "colorScheme": "Dark", "strokeWidth": 0,
      "opacity": 100, "properties": { "pk": "pv" } } ], "description": "" }"#;
    let t = theme_from_slice(raw.as_bytes()).expect("probe theme reads");
    assert_eq!(t.description.as_deref(), Some(""));
    assert_eq!(t.elements[0].stroke_width, Some(1), "setter clamps to 1");
    let out = String::from_utf8(theme_write(&t)).unwrap();
    // `description: ""` is hidden (NON_EMPTY), key order follows declaration.
    assert_eq!(
        out,
        r#"{
  "elements" : [ {
    "tag" : "A",
    "colorScheme" : "Dark",
    "properties" : {
      "pk" : "pv"
    },
    "strokeWidth" : 1,
    "opacity" : 100
  } ]
}"#
    );
}

#[test]
fn empty_theme_is_an_empty_object() {
    assert_eq!(
        String::from_utf8(theme_write(&Theme::default())).unwrap(),
        "{ }"
    );
}

#[test]
fn name_and_description_lead_and_empty_ones_hide() {
    // `big-bank-plc-theme.json` shape: `name, description, elements` — no
    // `relationships` (empty, NON_EMPTY), no `license` (never set).
    let theme = Theme {
        name: Some("Big Bank plc".into()),
        description: Some("Main starting point".into()),
        elements: vec![ElementStyle {
            tag: "Person".into(),
            background: Some("#08427b".into()),
            ..Default::default()
        }],
        ..Default::default()
    };
    let out = String::from_utf8(theme_write(&theme)).unwrap();
    assert!(out.starts_with(
        "{\n  \"name\" : \"Big Bank plc\",\n  \"description\" : \"Main starting point\",\n  \"elements\" : [ {\n    \"tag\" : \"Person\","
    ), "{out}");
    assert!(
        !out.contains("relationships") && !out.contains("license"),
        "{out}"
    );
}
