//! Category A golden path (P3-D1): open a workspace through `oxidizr-api`,
//! build a scene and paint it to an `epaint` shape list — all headless
//! (`cargo test` never enables `gui`, so no GPU, no window). Guards the
//! M1 spine: `open` → `scene::build` → `paint::paint_scene` stay wired.

use std::path::PathBuf;

use epaint::text::{FontDefinitions, Fonts, TextOptions};
use epaint::{Rect as ERect, Shape, pos2};
use oxidizr_api::open;
use oxidizr_api::pipeline::{ApiOptions, Input};
use oxidizr_core::views::ColorScheme;
use oxidizr_viewer::{hit, paint, scene};

#[allow(clippy::unwrap_used)]
fn temp_dsl(topic: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oxidizr-m14-{topic}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("demo.dsl");
    std::fs::write(&path, text).unwrap();
    path
}

const DSL: &str = r#"workspace "Bank Demo" "M1-4 fixture" {
    model {
        alice = person "Alice" "A user" "Live"
        bank = softwaresystem "Bank" "Core banking" "Live"
        alice -> bank "Uses" "HTTPS"
    }
}"#;

/// Paint the workspace's first view in `scheme` and return the shapes.
#[allow(clippy::unwrap_used)]
fn paint_first_view(topic: &str, scheme: ColorScheme) -> (scene::Scene, Vec<Shape>) {
    let path = temp_dsl(topic, DSL);
    let opened = open(&Input::File(path), &ApiOptions::default()).unwrap();
    let key = opened
        .workspace
        .views
        .all_views()
        .first()
        .map(|view| view.key().to_owned())
        .unwrap();
    let scene = scene::build(&opened.workspace, &key, scheme, None).unwrap();
    let viewport = hit::Viewport::fit(scene.bounds, (1200.0, 800.0));
    let mut atlas = Fonts::new(TextOptions::default(), FontDefinitions::default());
    let shapes = {
        let mut fonts = atlas.with_pixels_per_point(1.0);
        paint::paint_scene(
            &scene,
            &viewport,
            &mut fonts,
            &paint::PaintOptions {
                screen: ERect::from_min_max(pos2(0.0, 0.0), pos2(1200.0, 800.0)),
                selection: None,
            },
        )
    };
    (scene, shapes)
}

fn texts(shapes: &[Shape]) -> String {
    shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Text(text) => Some(text.galley.text()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("|")
}

#[test]
#[allow(clippy::unwrap_used)]
fn open_build_paint_produces_labelled_boxes() {
    let (scene, shapes) = paint_first_view("labelled", ColorScheme::Light);
    // The default view includes both elements + their relationship.
    assert_eq!(scene.items.len(), 2, "Alice and the Bank");
    assert_eq!(scene.edges.len(), 1, "Alice -> Bank");
    // Painting produced geometry AND the element/edge text runs.
    assert!(shapes.len() > scene.items.len(), "geometry painted");
    let painted = texts(&shapes);
    assert!(painted.contains("Alice"), "name painted: {painted}");
    assert!(painted.contains("Bank"), "system painted: {painted}");
    assert!(painted.contains("Uses"), "relationship label painted");
}

#[test]
fn paint_is_deterministic_across_runs() {
    let (_, a) = paint_first_view("det-a", ColorScheme::Light);
    let (_, b) = paint_first_view("det-b", ColorScheme::Light);
    assert_eq!(a.len(), b.len());
    assert_eq!(texts(&a), texts(&b));
}

#[test]
fn dark_and_light_schemes_both_paint() {
    let (light_scene, light) = paint_first_view("schemes-l", ColorScheme::Light);
    let (dark_scene, dark) = paint_first_view("schemes-d", ColorScheme::Dark);
    // Same model → same item count; a different background per scheme.
    assert_eq!(light_scene.items.len(), dark_scene.items.len());
    assert_ne!(light_scene.background, dark_scene.background);
    assert!(!light.is_empty() && !dark.is_empty());
    // Both are fully within the painted screen rect (fit guarantees it).
    let screen = ERect::from_min_max(pos2(-1.0, -1.0), pos2(1201.0, 801.0));
    for shape in &dark {
        let b = shape.visual_bounding_rect();
        assert!(
            screen.contains(b.min) && screen.contains(b.max),
            "a dark-scheme shape escaped the fitted viewport: {shape:?} -> {b:?}"
        );
    }
}
