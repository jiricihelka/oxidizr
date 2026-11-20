//! The auto-layout seam (P3-M2-5): scene → neutral [`LayoutGraph`] →
//! the §P3-D5 MVP engine (`oxidizr_layout::Layered`) → [`Overrides`]
//! that [`scene::build`] overlays on the stored geometry. The GUI shell
//! (feature `gui`) only wires button + dialog + the ONE pass — every
//! decision lives here, tested headlessly (boundary guard, P3-D1).
//!
//! Design facts (the §P3-M2 M2-5 row):
//!
//! - [`graph_for`] reads **ids and sizes only** from the scene — never
//!   positions — so a run with the same config is IDEMPOTENT: re-running
//!   on an already-overridden scene reproduces identical [`Overrides`].
//! - Boundaries need no override entries: [`scene::build`] recomputes
//!   them from member unions, so they follow moved items (V1). The
//!   boundary pad stays [`crate::scene::BOUNDARY_PADDING`] (the M1
//!   chrome constant, not dagre's internal 50 px — the deviation
//!   recorded on the M2-5 row).
//! - Engine node placements are box TOP-LEFTS in the scene's frame;
//!   route points are world-space polylines whose FIRST and LAST points
//!   are endpoint anchors — those are re-derived from the (moved) boxes
//!   by the scene, so only `points[1..len-1]` become interior waypoints.
//! - The engine ignores `edge_separation` at MVP scope; the dialog
//!   keeps the field for reference parity (`auto-layout.jspf`).

use oxidizr_core::Id;
use oxidizr_core::Workspace;
use oxidizr_core::views::{AutomaticLayout, RankDirection, View};
use oxidizr_layout::{
    Edge as GraphEdge, EdgeId, Layered, LayoutConfig, LayoutEngine, LayoutError, LayoutGraph,
    Node as GraphNode, NodeId, RankDirection as EngineDirection,
};

use crate::scene::{Overrides, Scene};

/// The engine config a stored `automaticLayout` (or the dialog's
/// working copy) maps to. A null `rankDirection` falls to the reference
/// default, exactly like the core's [`AutomaticLayout::default`].
#[must_use]
pub fn config_for(layout: &AutomaticLayout) -> LayoutConfig {
    LayoutConfig {
        rank_direction: layout
            .rank_direction
            .map_or_else(EngineDirection::default, direction),
        rank_separation: layout.rank_separation,
        node_separation: layout.node_separation,
        edge_separation: layout.edge_separation,
        generate_vertices: layout.vertices,
    }
}

/// The contract's [`RankDirection`] onto the engine's (names differ:
/// core mirrors the JSON keys, the layout crate the dialog labels).
#[must_use]
pub const fn direction(value: RankDirection) -> EngineDirection {
    match value {
        RankDirection::TopBottom => EngineDirection::TopDown,
        RankDirection::BottomTop => EngineDirection::BottomUp,
        RankDirection::LeftRight => EngineDirection::LeftRight,
        RankDirection::RightLeft => EngineDirection::RightLeft,
    }
}

/// The stored `automaticLayout` of one view key — for a FILTERED view
/// its base's (that is the rendered content, same resolution as
/// [`crate::scene::build`]). `None` for unknown keys and views without
/// a layout request.
#[must_use]
pub fn stored_for(workspace: &Workspace, view_key: &str) -> Option<AutomaticLayout> {
    let views = workspace.views.all_views();
    let view = views.iter().find(|v| v.key() == view_key).copied()?;
    let rendered: View = match view {
        View::Filtered(f) => views.iter().find(|v| v.key() == f.base_view_key).copied()?,
        other => other,
    };
    rendered.content()?.automatic_layout
}

/// The layout graph of `scene`: one node per ITEM (id + the
/// style-resolved box size), one edge per relationship edge between
/// items (the scene already dropped boundary/unresolvable endpoints).
/// Content order = insertion order (deterministic, §P3-D5).
#[must_use]
pub fn graph_for(scene: &Scene, config: LayoutConfig) -> LayoutGraph {
    let mut graph = LayoutGraph::new();
    graph.config = config;
    for item in &scene.items {
        graph.add_node(GraphNode::new(
            NodeId::new(item.id.as_str()),
            item.rect.width,
            item.rect.height,
        ));
    }
    for edge in &scene.edges {
        graph.add_edge(GraphEdge::new(
            EdgeId::new(edge.id.as_str()),
            NodeId::new(edge.source.as_str()),
            NodeId::new(edge.destination.as_str()),
        ));
    }
    graph
}

/// Run the MVP engine over `scene` and translate the result into scene
/// [`Overrides`]: node placements become element top-lefts, route
/// interiors become relationship waypoints (module docs).
///
/// # Errors
///
/// Any [`LayoutError`] from graph validation — unreachable for a scene
/// built by [`crate::scene::build`] (unique item ids, finite positive
/// sizes), but errors stay values (P3 charter).
pub fn apply(scene: &Scene, layout: &AutomaticLayout) -> Result<Overrides, LayoutError> {
    let result = Layered::new().layout(&graph_for(scene, config_for(layout)))?;
    let mut overrides = Overrides::default();
    for place in &result.nodes {
        overrides.elements.insert(
            Id::new(place.id.as_str()),
            (place.position.x, place.position.y),
        );
    }
    for route in &result.edges {
        let interior: Vec<(f64, f64)> = if route.points.len() >= 2 {
            route.points[1..route.points.len() - 1]
                .iter()
                .map(|p| (p.x, p.y))
                .collect()
        } else {
            Vec::new()
        };
        overrides
            .relationships
            .insert(Id::new(route.id.as_str()), interior);
    }
    Ok(overrides)
}

/// The persisted form of the dialog parameters (§P3-D2: eframe storage
/// carries PARAMETERS only — the produced geometry is session state):
/// `"LR|100|50|50|1"`.
#[must_use]
pub fn encode(layout: &AutomaticLayout) -> String {
    let direction = match layout.rank_direction.unwrap_or_default() {
        RankDirection::TopBottom => "TB",
        RankDirection::BottomTop => "BT",
        RankDirection::LeftRight => "LR",
        RankDirection::RightLeft => "RL",
    };
    format!(
        "{direction}|{}|{}|{}|{}",
        layout.rank_separation,
        layout.node_separation,
        layout.edge_separation,
        u8::from(layout.vertices),
    )
}

/// [`encode`]'s inverse; malformed input is `None` so the caller falls
/// back to [`AutomaticLayout::default`].
#[must_use]
pub fn decode(text: &str) -> Option<AutomaticLayout> {
    let mut parts = text.split('|');
    let direction = match parts.next()? {
        "TB" => RankDirection::TopBottom,
        "BT" => RankDirection::BottomTop,
        "LR" => RankDirection::LeftRight,
        "RL" => RankDirection::RightLeft,
        _ => return None,
    };
    let number = |part: Option<&str>| part?.parse::<i32>().ok();
    let rank_separation = number(parts.next())?;
    let node_separation = number(parts.next())?;
    let edge_separation = number(parts.next())?;
    let vertices = match number(parts.next())? {
        0 => false,
        1 => true,
        _ => return None,
    };
    Some(AutomaticLayout {
        rank_direction: Some(direction),
        rank_separation,
        node_separation,
        edge_separation,
        vertices,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::scene;
    use oxidizr_core::Workspace;
    use oxidizr_core::model::{ModelItem, Relationship};
    use oxidizr_core::tags::Tags;
    use oxidizr_core::views::{
        ColorScheme, ElementView, SystemLandscapeView, ViewBase, ViewContent,
    };
    use std::collections::HashMap;

    fn id(n: u32) -> Id {
        Id::new(n.to_string())
    }

    fn rel(n: u32, from: u32, to: u32) -> Relationship {
        Relationship {
            id: id(n),
            source: id(from),
            destination: id(to),
            description: Some("Uses".to_owned()),
            technology: Some("HTTPS".to_owned()),
            tags: Tags::new(),
            url: None,
            properties: oxidizr_core::properties::Properties::new(),
            perspectives: Vec::new(),
            interaction_style: None,
            linked_relationship_id: None,
        }
    }

    fn rel_view(n: u32) -> oxidizr_core::views::RelationshipView {
        oxidizr_core::views::RelationshipView {
            id: id(n),
            description: None,
            url: None,
            order: None,
            response: None,
            properties: oxidizr_core::properties::Properties::new(),
            vertices: Vec::new(),
            routing: None,
            jump: None,
            position: None,
        }
    }

    fn element(n: u32, x: i32, y: i32) -> ElementView {
        ElementView { id: id(n), x, y }
    }

    /// `chain(3)`: `a(1) → b(2) → c(3)` systems, view `L1`, stored
    /// geometry deliberately unsorted so the engine has real work.
    fn chain(automatic: Option<AutomaticLayout>) -> Workspace {
        let mut ws = Workspace::default();
        for (n, name) in [(1, "a"), (2, "b"), (3, "c")] {
            ws.model
                .software_systems
                .push(oxidizr_core::model::SoftwareSystem {
                    item: ModelItem::new(id(n), name),
                    group: None,
                    containers: Vec::new(),
                    documentation: oxidizr_core::docs::Documentation::default(),
                    relationships: Vec::new(),
                });
        }
        ws.model.relationships.push(rel(10, 1, 2));
        ws.model.relationships.push(rel(11, 2, 3));
        ws.views.system_landscape_views.push(SystemLandscapeView {
            base: ViewBase::new("L1", 1),
            content: ViewContent {
                automatic_layout: automatic,
                elements: vec![
                    element(1, 500, 500),
                    element(2, 100, 100),
                    element(3, 300, 0),
                ],
                relationships: vec![rel_view(10), rel_view(11)],
                ..ViewContent::default()
            },
            enterprise_boundary_visible: false,
            animations: Vec::new(),
        });
        ws
    }

    fn scene_of(ws: &Workspace, overrides: Option<&Overrides>) -> Scene {
        scene::build(ws, "L1", ColorScheme::Light, None, overrides).unwrap()
    }

    #[test]
    fn config_maps_the_reference_defaults() {
        let config = config_for(&AutomaticLayout::default());
        assert_eq!(config, LayoutConfig::default());
        assert_eq!(
            config.rank_direction,
            EngineDirection::LeftRight,
            "null direction falls to LR"
        );
        assert_eq!(
            config_for(&AutomaticLayout {
                rank_direction: Some(RankDirection::BottomTop),
                rank_separation: 10,
                node_separation: 20,
                edge_separation: 30,
                vertices: false,
            }),
            LayoutConfig {
                rank_direction: EngineDirection::BottomUp,
                rank_separation: 10,
                node_separation: 20,
                edge_separation: 30,
                generate_vertices: false,
            }
        );
        assert_eq!(
            direction(RankDirection::TopBottom),
            EngineDirection::TopDown
        );
        assert_eq!(
            direction(RankDirection::RightLeft),
            EngineDirection::RightLeft
        );
    }

    #[test]
    fn stored_for_reads_views_and_filtered_bases() {
        let automatic = AutomaticLayout {
            rank_direction: Some(RankDirection::TopBottom),
            ..AutomaticLayout::default()
        };
        let ws = chain(Some(automatic));
        assert_eq!(stored_for(&ws, "L1"), Some(automatic));
        // A filtered view reads its BASE's request.
        let mut filtered = ws.clone();
        filtered
            .views
            .filtered_views
            .push(oxidizr_core::views::FilteredView {
                base: ViewBase::new("F1", 2),
                base_view_key: "L1".to_string(),
                mode: None,
                tags: Vec::new(),
            });
        assert_eq!(stored_for(&filtered, "F1"), Some(automatic));
        assert_eq!(stored_for(&ws, "nope"), None);
        assert_eq!(stored_for(&chain(None), "L1"), None);
    }

    #[test]
    fn apply_lays_the_chain_out_left_to_right() {
        let ws = chain(None);
        let overrides = apply(&scene_of(&ws, None), &AutomaticLayout::default()).unwrap();
        // 350×150 default boxes, LR/100/50: ranks advance by 450, the
        // single-node ranks share the centre line — exact floats.
        let expected: HashMap<Id, (f64, f64)> =
            [(1, (0.0, 0.0)), (2, (450.0, 0.0)), (3, (900.0, 0.0))]
                .map(|(n, p)| (id(n), p))
                .into_iter()
                .collect();
        assert_eq!(overrides.elements, expected);
        // Straight routes: the two-point anchor legs leave no interior.
        assert_eq!(overrides.relationships.len(), 2);
        assert_eq!(overrides.relationships[&id(10)], [] as [(f64, f64); 0]);
        assert_eq!(overrides.relationships[&id(11)], [] as [(f64, f64); 0]);
        // The scene rebuild lands every item at its engine position.
        let laid = scene_of(&ws, Some(&overrides));
        for (n, (x, y)) in expected {
            let item = laid.items.iter().find(|i| i.id == n).unwrap();
            assert_eq!((item.rect.x, item.rect.y), (x, y));
        }
        assert_eq!(
            laid.edges[0].points,
            vec![(175.0, 75.0), (625.0, 75.0)],
            "centres re-derived from the moved boxes, no interior"
        );
    }

    #[test]
    // Exact layout values (integer separations, exact halves): the
    // axis-alignment check IS equality by design (§P3-D5 determinism).
    #[allow(clippy::float_cmp)]
    fn long_edge_gets_waypoints_in_the_scene() {
        // a → b → c PLUS a → c spanning two ranks: the scene polyline
        // of `ac` keeps the engine's rank-gap channel.
        let mut ws = chain(None);
        ws.model.relationships.push(rel(12, 1, 3));
        ws.views.system_landscape_views[0]
            .content
            .relationships
            .push(rel_view(12));
        let overrides = apply(&scene_of(&ws, None), &AutomaticLayout::default()).unwrap();
        let ac = &overrides.relationships[&id(12)];
        assert!(!ac.is_empty(), "the spanning edge keeps channel points");
        let laid = scene_of(&ws, Some(&overrides));
        let edge = laid.edges.iter().find(|e| e.id == id(12)).unwrap();
        assert!(edge.points.len() >= 4);
        for pair in edge.points.windows(2) {
            assert!(
                pair[0].0 == pair[1].0 || pair[0].1 == pair[1].1,
                "scene leg {:?}→{:?} is diagonal",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn reapplying_is_idempotent() {
        // The graph reads ids + sizes only: the laid-out scene re-runs
        // to identical overrides (no layout drift on repeat opens).
        let ws = chain(None);
        let first = apply(&scene_of(&ws, None), &AutomaticLayout::default()).unwrap();
        let second = apply(&scene_of(&ws, Some(&first)), &AutomaticLayout::default()).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn straight_config_keeps_centres_only() {
        let ws = chain(None);
        let straight = AutomaticLayout {
            vertices: false,
            ..AutomaticLayout::default()
        };
        let overrides = apply(&scene_of(&ws, None), &straight).unwrap();
        assert!(
            overrides.relationships.values().all(Vec::is_empty),
            "generate_vertices=false ⇒ no interior points"
        );
    }

    #[test]
    fn params_round_trip_and_reject_garbage() {
        for direction in [
            RankDirection::TopBottom,
            RankDirection::BottomTop,
            RankDirection::LeftRight,
            RankDirection::RightLeft,
        ] {
            let params = AutomaticLayout {
                rank_direction: Some(direction),
                rank_separation: 90,
                node_separation: 40,
                edge_separation: 30,
                vertices: direction == RankDirection::BottomTop,
            };
            assert_eq!(decode(&encode(&params)), Some(params));
        }
        assert_eq!(decode(""), None);
        assert_eq!(decode("XX|100|50|50|1"), None);
        assert_eq!(decode("LR|100|50"), None);
        assert_eq!(decode("LR|100|50|50|2"), None);
        assert_eq!(decode("LR|wide|50|50|1"), None);
    }
}
