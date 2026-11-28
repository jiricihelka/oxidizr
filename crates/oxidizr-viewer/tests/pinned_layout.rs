//! §M3-V4 pinned tidy, end-to-end headless: the manual overlay entries
//! ARE the pins, and a re-Apply holds them EXACTLY — byte-for-byte, the
//! coordinates the user dragged to come back unchanged while the rest of
//! the view re-runs. Runs on a real corpus view (a grouped Components
//! view under the default compound engine), so the engine switch the
//! pin map forces (compound → built-in) is part of the covered path.

use std::collections::HashMap;
use std::path::PathBuf;

use oxidizr_api::open;
use oxidizr_api::pipeline::{ApiOptions, Input};
use oxidizr_core::views::{AutomaticLayout, ColorScheme};
use oxidizr_viewer::{layout, scene};

#[allow(clippy::unwrap_used)]
fn dsl(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/dsl")
        .join(name)
        .canonicalize()
        .unwrap()
}

#[test]
#[allow(clippy::unwrap_used)]
fn pinned_elements_survive_apply_byte_exactly() {
    let opened = open(
        &Input::File(dsl("big-bank-plc.dsl")),
        &ApiOptions::default(),
    )
    .unwrap();
    let workspace = &opened.workspace;
    let key = "Components";
    let params = AutomaticLayout::default();

    // Lay the view out once, then treat TWO placed elements as if the
    // user had dragged them: one stays where the engine put it, the
    // other moves to an off-grid position. Both are overlay entries —
    // both are pins.
    let raw = scene::build(workspace, key, ColorScheme::Light, None, None).unwrap();
    let laid = layout::apply(&raw, &params).unwrap();
    let mut pins: HashMap<oxidizr_core::Id, (f64, f64)> = HashMap::new();
    let ids: Vec<oxidizr_core::Id> = raw.items.iter().map(|item| item.id.clone()).collect();
    assert!(ids.len() >= 4, "the fixture view must have content");
    let held = laid.elements[&ids[0]];
    let dragged = (777.5, -123.25);
    pins.insert(ids[0].clone(), held);
    pins.insert(ids[1].clone(), dragged);
    let mut seeded = laid;
    seeded.elements.extend(pins.clone());
    let scene_seeded =
        scene::build(workspace, key, ColorScheme::Light, None, Some(&seeded)).unwrap();

    // Apply with the pins: the pinned boxes return VERBATIM.
    let out =
        layout::apply_pinned(layout::Engine::default(), &scene_seeded, &params, &pins).unwrap();
    assert_eq!(out.elements[&ids[0]], held, "engine-pinned box drifted");
    assert_eq!(out.elements[&ids[1]], dragged, "user-dragged pin drifted");

    // The scene built from the result carries the pins byte-exactly…
    let rebuilt = scene::build(workspace, key, ColorScheme::Light, None, Some(&out)).unwrap();
    for id in pins.keys() {
        let item = rebuilt.items.iter().find(|i| i.id == *id).unwrap();
        let (x, y) = pins[id];
        assert_eq!((item.rect.x, item.rect.y), (x, y), "{id}");
    }
    // …and one more Apply under the same pins is bit-identical.
    let again = layout::apply_pinned(layout::Engine::default(), &rebuilt, &params, &pins).unwrap();
    assert_eq!(out, again, "pinned re-Apply drifted");
}
