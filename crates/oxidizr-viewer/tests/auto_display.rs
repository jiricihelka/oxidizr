//! §P3-M6 display auto-layout, end-to-end on the vendored corpus: the
//! big-bank-plc DSL carries NO `location` statements — before the display
//! pass shipped, the viewer opened EVERY such view as a pile at `(0, 0)`
//! (the loader materializes a missing location as that pair). Guards the
//! chain the shell rides: raw build = pile → `auto_display` spreads it →
//! the user overlay pins on top → `seed_overlay` persists what was shown.

use std::path::PathBuf;

use oxidizr_api::open;
use oxidizr_api::pipeline::{ApiOptions, Input};
use oxidizr_core::views::ColorScheme;
use oxidizr_viewer::{commands, layout, scene};

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
fn un_laid_out_big_bank_components_view_opens_spread_out() {
    let opened = open(
        &Input::File(dsl("big-bank-plc.dsl")),
        &ApiOptions::default(),
    )
    .unwrap();
    let workspace = &opened.workspace;
    let key = "Components";

    // The raw stored geometry is the pile: every element sits at 0,0.
    let raw = scene::build(workspace, key, ColorScheme::Light, None, None).unwrap();
    assert!(layout::view_needs_layout(workspace, key));
    assert!(
        raw.items
            .iter()
            .all(|item| item.rect.x == 0.0 && item.rect.y == 0.0),
        "the fixture must still be the un-laid-out pile"
    );
    assert!(raw.items.len() >= 5, "the Components view has real content");

    // The display pass covers EVERY visible item and spreads them out.
    let auto = layout::auto_display(workspace, key, &raw).unwrap();
    assert_eq!(auto.elements.len(), raw.items.len());
    let spread = scene::build(workspace, key, ColorScheme::Light, None, Some(&auto)).unwrap();
    let mut xs: Vec<f64> = spread.items.iter().map(|item| item.rect.x).collect();
    xs.sort_unstable_by(f64::total_cmp);
    xs.dedup();
    assert!(xs.len() >= 3, "the engine spreads the boxes across ranks");

    // The user overlay merges ON TOP of the engine geometry (per id).
    let mut overlay = commands::LayoutOverlay::for_view(key);
    let pin = spread.items[0].id.clone();
    overlay.elements.insert(pin.clone(), (7.0, 7.0));
    let merged = scene::Overrides::display(&auto, Some(&overlay.as_overrides()));
    let shown = scene::build(workspace, key, ColorScheme::Light, None, Some(&merged)).unwrap();
    let pinned = shown.items.iter().find(|item| item.id == pin).unwrap();
    assert_eq!((pinned.rect.x, pinned.rect.y), (7.0, 7.0));
    let others = shown
        .items
        .iter()
        .filter(|item| item.id != pin)
        .filter(|item| {
            spread
                .items
                .iter()
                .any(|s| s.id == item.id && s.rect == item.rect)
        })
        .count();
    assert_eq!(others, shown.items.len() - 1, "the rest keep the engine");

    // Saving what the user SAW: the seed fills the untouched ids with
    // the engine geometry, the pinned id keeps the user's placement.
    let seeded = layout::seed_overlay(workspace, key, &overlay).unwrap();
    assert_eq!(seeded.elements.get(&pin), Some(&(7.0, 7.0)));
    assert_eq!(seeded.elements.len(), raw.items.len());
}
