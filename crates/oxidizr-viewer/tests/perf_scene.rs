//! P3-M5 perf pass (M5-2, informational per P3-D0): scene rebuild +
//! paint budgets on the real corpora. IGNORED by default — numbers are
//! recorded in `agents/notes/perf/` (no gate), re-run with:
//!
//! ```text
//! cargo test --release -p oxidizr-viewer --test perf_scene -- --ignored --nocapture
//! ```
//!
//! The budget from §P3-M5: 60 fps (16.7 ms/frame) with ≥ 200 visible
//! elements — paint is the per-frame hot path, a rebuild only rides an
//! edit or a reload. Everything here is headless (module docs of
//! `golden_paint.rs`): `open` → `scene::build` → `paint::paint_scene`.

use std::path::{Path, PathBuf};
use std::time::Instant;

use epaint::text::{FontDefinitions, Fonts, TextOptions};
use epaint::{Rect as ERect, pos2};
use oxidizr_api::open;
use oxidizr_api::pipeline::{ApiOptions, Input};
use oxidizr_core::Id;
use oxidizr_core::views::ColorScheme;
use oxidizr_viewer::{hit, paint, scene};

/// ROUNDS ≥ 3 per figure (the timing-note rule); first round included —
/// the release build is warm enough at this granularity.
const ROUNDS: u32 = 5;

#[allow(clippy::unwrap_used)]
fn repo_testdata_dsl() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/dsl")
        .canonicalize()
        .unwrap()
}

/// The largest scene across a workspace's views: (key, items, edges).
#[allow(clippy::unwrap_used)]
fn largest_view(workspace: &oxidizr_core::Workspace) -> (String, usize, usize) {
    let mut best = (String::new(), 0_usize, 0_usize);
    for view in workspace.views.all_views() {
        let Ok(built) = scene::build(workspace, view.key(), ColorScheme::Light, None, None) else {
            continue;
        };
        if built.items.len() + built.edges.len() > best.1 + best.2 {
            best = (view.key().to_owned(), built.items.len(), built.edges.len());
        }
    }
    best
}

#[test]
#[ignore = "P3-M5 perf pass — informational, run --release -- --ignored --nocapture"]
#[allow(clippy::unwrap_used)]
fn perf_scene_rebuild_and_paint() {
    let root = repo_testdata_dsl();
    let mut atlas = Fonts::new(TextOptions::default(), FontDefinitions::default());
    for (label, rel) in [
        ("big-bank-plc", "big-bank-plc.dsl"),
        ("spring-petclinic", "spring-petclinic/workspace.dsl"),
        ("amazon-web-services", "amazon-web-services.dsl"),
    ] {
        let path = root.join(rel);
        let opened = open(&Input::File(path), &ApiOptions::default()).unwrap();
        let workspace = &opened.workspace;
        let (key, items, edges) = largest_view(workspace);
        if key.is_empty() {
            println!("{label}: NO buildable view — skipped");
            continue;
        }

        // Rebuild: every view ROUNDS times (an edit/reload rebuilds the
        // current view; this is the pessimistic all-views-per-frame form).
        let keys: Vec<String> = workspace
            .views
            .all_views()
            .iter()
            .map(|view| view.key().to_owned())
            .collect();
        let start = Instant::now();
        for _ in 0..ROUNDS {
            for k in &keys {
                let _ = scene::build(workspace, k, ColorScheme::Light, None, None).unwrap();
            }
        }
        let rebuild_all = start.elapsed();
        let builds = u32::try_from(ROUNDS as usize * keys.len()).unwrap();

        // Paint: the largest view, fitted viewport, full screen.
        let built = scene::build(workspace, &key, ColorScheme::Light, None, None).unwrap();
        let viewport = hit::Viewport::fit(built.bounds, (1920.0, 1080.0));
        let options = paint::PaintOptions {
            screen: ERect::from_min_max(pos2(0.0, 0.0), pos2(1920.0, 1080.0)),
            selection: None,
            hover_edge: None,
        };
        let start = Instant::now();
        for _ in 0..ROUNDS {
            let mut fonts = atlas.with_pixels_per_point(1.0);
            let shapes = paint::paint_scene(&built, &viewport, &mut fonts, &options);
            std::hint::black_box(shapes.len());
        }
        let paint_total = start.elapsed();

        println!(
            "{label}: views {} | largest {key} = {items} items + {edges} edges | \
             rebuild-all {:.3} ms/view ({builds} build(s)) | paint {:.3} ms/frame ({ROUNDS} frames)",
            keys.len(),
            rebuild_all.as_secs_f64() * 1000.0 / f64::from(builds),
            paint_total.as_secs_f64() * 1000.0 / f64::from(ROUNDS),
        );
    }
}

/// The §P3-M5 budget itself: ≥ 200 visible elements at 60 fps (≤ 16.7
/// ms/frame). The corpora top out at 24 visible elements, so tile the
/// largest big-bank-plc view into a grid until ≥ 200 items are on the
/// canvas — same content mix (text galley included), pessimistic scale.
#[test]
#[ignore = "P3-M5 perf pass — informational, run --release -- --ignored --nocapture"]
#[allow(clippy::unwrap_used)]
fn perf_paint_budget_200_elements() {
    let root = repo_testdata_dsl();
    let opened = open(
        &Input::File(root.join("big-bank-plc.dsl")),
        &ApiOptions::default(),
    )
    .unwrap();
    let key = largest_view(&opened.workspace).0;
    let base = scene::build(&opened.workspace, &key, ColorScheme::Light, None, None).unwrap();

    // Grid-tiling until ≥ 200 items; tile stride = base bounds + 200 px.
    let per = base.items.len().max(1);
    let tile_count = 200_usize.div_ceil(per);
    let mut cols = 1_usize;
    while cols * cols < tile_count {
        cols += 1;
    }
    let rows = tile_count.div_ceil(cols);
    let stride_x = base.bounds.width + 200.0;
    let stride_y = base.bounds.height + 200.0;
    let mut items = Vec::new();
    let mut edges = Vec::new();
    for t in 0..tile_count {
        let dx = f64::from(u32::try_from(t % cols).unwrap()) * stride_x;
        let dy = f64::from(u32::try_from(t / cols).unwrap()) * stride_y;
        for item in &base.items {
            let mut copy = item.clone();
            copy.id = Id::new(format!("{}-t{t}", item.id.as_str()));
            copy.rect.x += dx;
            copy.rect.y += dy;
            items.push(copy);
        }
        for edge in &base.edges {
            let mut copy = edge.clone();
            copy.id = Id::new(format!("{}-t{t}", edge.id.as_str()));
            for point in &mut copy.points {
                point.0 += dx;
                point.1 += dy;
            }
            edges.push(copy);
        }
    }
    let bounds = scene::Rect {
        x: base.bounds.x,
        y: base.bounds.y,
        width: base.bounds.width + f64::from(u32::try_from(cols - 1).unwrap()) * stride_x,
        height: base.bounds.height + f64::from(u32::try_from(rows - 1).unwrap()) * stride_y,
    };
    let stress = scene::Scene {
        view_key: format!("{key}-stress"),
        title: base.title.clone(),
        background: base.background,
        boundaries: Vec::new(),
        items,
        edges,
        bounds,
    };

    let mut atlas = Fonts::new(TextOptions::default(), FontDefinitions::default());
    let viewport = hit::Viewport::fit(stress.bounds, (1920.0, 1080.0));
    let options = paint::PaintOptions {
        screen: ERect::from_min_max(pos2(0.0, 0.0), pos2(1920.0, 1080.0)),
        selection: None,
        hover_edge: None,
    };
    let start = Instant::now();
    for _ in 0..ROUNDS {
        let mut fonts = atlas.with_pixels_per_point(1.0);
        let shapes = paint::paint_scene(&stress, &viewport, &mut fonts, &options);
        std::hint::black_box(shapes.len());
    }
    let paint_total = start.elapsed();
    println!(
        "budget-200: {} items + {} edges visible | fit scale {:.3} | paint {:.3} ms/frame \
         vs 16.7 ms (60 fps) budget ({ROUNDS} frames)",
        stress.items.len(),
        stress.edges.len(),
        viewport.scale(),
        paint_total.as_secs_f64() * 1000.0 / f64::from(ROUNDS),
    );
}
