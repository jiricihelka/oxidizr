//! §M2-V3 compound layout, end-to-end on the vendored corpus: every view
//! that renders a boundary (group box, deployment node, parent scope)
//! runs the auto-layout seam with the group forest in the graph. Three
//! promises per view: the boundary frames match their member geometry,
//! edges keep attached endpoints, and re-Applying reproduces byte-
//! identical overrides — plus the V2 baseline: `Layered` output never
//! sees the groups (`graph_for` is the only place they are emitted).

use std::path::PathBuf;

use oxidizr_api::open;
use oxidizr_api::pipeline::{ApiOptions, Input};
use oxidizr_core::views::{AutomaticLayout, ColorScheme};
use oxidizr_viewer::{layout, scene};

#[allow(clippy::unwrap_used)]
fn repo_testdata_dsl() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/dsl")
        .canonicalize()
        .unwrap()
}

#[allow(clippy::unwrap_used)]
fn dsl_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(repo_testdata_dsl()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "dsl") {
            files.push(path);
        }
    }
    files.sort();
    files
}

/// The boundary frame equals its member union + the scene pad: the
/// `members` list must be exactly the closure the rect was drawn from
/// (deployment-node items transitively included).
#[allow(clippy::float_cmp)]
fn assert_frames_match_members(scene: &scene::Scene) {
    for boundary in &scene.boundaries {
        assert!(!boundary.members.is_empty(), "{}: memberless", boundary.id);
        let mut union: Option<scene::Rect> = None;
        for member in &boundary.members {
            let item = scene
                .items
                .iter()
                .find(|i| &i.id == member)
                .unwrap_or_else(|| panic!("{}: member {} not an item", boundary.id, member));
            union = Some(union.map_or(item.rect, |acc| acc.union(item.rect)));
        }
        let frame = union.unwrap().expand(scene::BOUNDARY_PADDING);
        assert!(
            (frame.x - boundary.rect.x).abs() < 1e-6
                && (frame.y - boundary.rect.y).abs() < 1e-6
                && (frame.width - boundary.rect.width).abs() < 1e-6
                && (frame.height - boundary.rect.height).abs() < 1e-6,
            "{}: frame {:?} is not the member union {frame:?}",
            boundary.id,
            boundary.rect
        );
    }
}

/// Every laid-out edge still runs box-side to box-side: at least the two
/// anchors, all finite.
fn assert_edges_attached(scene: &scene::Scene) {
    for edge in &scene.edges {
        assert!(edge.points.len() >= 2, "{}: detached", edge.id);
        assert!(
            edge.points
                .iter()
                .all(|(x, y)| x.is_finite() && y.is_finite()),
            "{}: non-finite route",
            edge.id
        );
    }
}

#[test]
#[allow(clippy::unwrap_used)]
fn grouped_views_apply_idempotently_across_the_corpus() {
    let params = AutomaticLayout::default();
    let mut seen_views = 0usize;
    for file in dsl_files() {
        let Ok(opened) = open(&Input::File(file.clone()), &ApiOptions::default()) else {
            continue; // corpus files that do not open are not our business here
        };
        let workspace = &opened.workspace;
        for view in workspace.views.all_views() {
            let key = view.key().to_owned();
            let Ok(scene) = scene::build(workspace, &key, ColorScheme::Light, None, None) else {
                continue;
            };
            if scene.boundaries.is_empty() || scene.items.len() < 2 {
                continue; // the compound legs need both a frame and a pair
            }
            seen_views += 1;
            assert_frames_match_members(&scene);

            // The default engine (Dagre with the feature, the built-in
            // Layered without): every item placed, edges attached, and
            // re-Apply reproduces the overrides byte-identically.
            let first = layout::apply(&scene, &params)
                .unwrap_or_else(|error| panic!("{} {key}: apply: {error}", file.display()));
            assert_eq!(first.elements.len(), scene.items.len(), "{key}: coverage");
            let laid =
                scene::build(workspace, &key, ColorScheme::Light, None, Some(&first)).unwrap();
            assert_edges_attached(&laid);
            assert_frames_match_members(&laid);
            let second = layout::apply(&laid, &params).unwrap();
            assert_eq!(first, second, "{key}: re-Apply drifted");
        }
    }
    assert!(seen_views >= 5, "the corpus must offer group views");
}
