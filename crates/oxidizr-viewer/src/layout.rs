//! The auto-layout seam: scene → neutral [`LayoutGraph`] →
//! the bundled engine (`oxidizr_layout::Layered`) → [`Overrides`]
//! that [`crate::scene::build`] overlays on the stored geometry. The GUI shell
//! (feature `gui`) only wires button + dialog + the ONE pass — every
//! decision lives here, tested headlessly (boundary guard).
//!
//! - `graph_for` reads **ids and sizes only** from the scene — never
//!   positions — so a run with the same config is IDEMPOTENT: re-running
//!   on an already-overridden scene reproduces identical [`Overrides`].
//! - Boundaries need no override entries: [`crate::scene::build`] recomputes
//!   them from member unions, so they follow moved items. The
//!   boundary pad stays `crate::scene::BOUNDARY_PADDING` (our chrome
//!   constant, not the engine's internal 50 px).
//! - Engine node placements are box TOP-LEFTS in the scene's frame;
//!   route points are world-space polylines whose FIRST and LAST points
//!   are endpoint anchors — those are re-derived from the (moved) boxes
//!   by the scene, so only `points[1..len-1]` become interior waypoints.
//! - The engine ignores `edge_separation`; the dialog keeps the field
//!   because the stored format has it.
//! - The ENGINE is a session choice ([`Engine`]) — the dialog
//!   combo lists [`Engine::available`], so graphviz `dot` appears only
//!   when the `dot-backend` feature is compiled in AND `$OXIDZR_DOT_BIN`
//!   names a binary. The choice is deliberately NOT persisted:
//!   `encode`/`decode` keep their five-field `"LR|100|50|50|1"` format.

use oxidizr_core::Id;
use oxidizr_core::Workspace;
use oxidizr_core::views::{AutomaticLayout, RankDirection, View};
#[cfg(feature = "dot-backend")]
use oxidizr_layout::Dot;
use oxidizr_layout::{
    Edge as GraphEdge, EdgeId, Layered, LayoutConfig, LayoutEngine, LayoutError, LayoutGraph,
    LayoutResult, Node as GraphNode, NodeId, RankDirection as EngineDirection,
};

use crate::scene::{Overrides, Scene};

/// Which engine a layout run uses. `Layered` is the bundled
/// engine; `Dot` exists only behind the viewer's
/// `dot-backend` feature and shells to graphviz.
/// Session-only: the selection is never persisted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Engine {
    /// The bundled layered engine — always available.
    #[default]
    Layered,
    /// The external graphviz `dot` (feature `dot-backend` only).
    #[cfg(feature = "dot-backend")]
    Dot,
}

impl Engine {
    /// The engines THIS build+environment can actually run: `Layered`
    /// always, `Dot` only when compiled in and `$OXIDZR_DOT_BIN` is
    /// set (the dialog lists exactly these).
    #[must_use]
    pub fn available() -> Vec<Self> {
        // Shape avoids `mut` in the feature-OFF build (no unused-mut).
        #[cfg(feature = "dot-backend")]
        if Dot::from_env().is_some() {
            return vec![Self::Layered, Self::Dot];
        }
        vec![Self::Layered]
    }

    /// The dialog label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Layered => "Layered",
            #[cfg(feature = "dot-backend")]
            Self::Dot => "Graphviz dot",
        }
    }
}

/// The engine config a stored `automaticLayout` (or the dialog's
/// working copy) maps to. A null `rankDirection` falls to the default
/// direction, exactly like the core's [`AutomaticLayout::default`].
#[must_use]
pub(crate) fn config_for(layout: &AutomaticLayout) -> LayoutConfig {
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
pub(crate) fn stored_for(workspace: &Workspace, view_key: &str) -> Option<AutomaticLayout> {
    let views = workspace.views.all_views();
    let view = views.iter().find(|v| v.key() == view_key).copied()?;
    let rendered: View = match view {
        View::Filtered(f) => views.iter().find(|v| v.key() == f.base_view_key).copied()?,
        other => other,
    };
    rendered.content()?.automatic_layout
}

/// Does the RENDERED view (filtered → base, as `stored_for` and
/// `scene::build` resolve it) carry NO stored coordinates at all? The
/// loader materializes a missing `location` as the `0`/`0` pair, so
/// all-zeros means "the file has no layout for this view" — such a view
/// opens laid out by the bundled engine as a session DISPLAY, never
/// dirty.
#[must_use]
pub fn view_needs_layout(workspace: &Workspace, view_key: &str) -> bool {
    let views = workspace.views.all_views();
    let Some(view) = views.iter().find(|v| v.key() == view_key).copied() else {
        return false;
    };
    let rendered = match view {
        View::Filtered(f) => views.iter().find(|b| b.key() == f.base_view_key).copied(),
        other => Some(other),
    };
    let Some(rendered) = rendered else {
        return false;
    };
    let Some(content) = rendered.content() else {
        return false;
    };
    !content.elements.is_empty()
        && content
            .elements
            .iter()
            .all(|element| element.x == 0 && element.y == 0)
}

/// Generous display fallback spacing (the JSON
/// defaults of 100/50 read as cramped to the bundled engine next to
/// 350 px-wide boxes). Applies ONLY when the rendered view carries no
/// `automaticLayout` at all — a stored one stays authoritative, file for
/// file. The geometry is session display state, so no byte is
/// touched and the core's [`AutomaticLayout::default`] (a JSON default)
/// stands untouched.
pub(crate) const DISPLAY_FALLBACK: AutomaticLayout = AutomaticLayout {
    rank_direction: Some(RankDirection::LeftRight),
    rank_separation: 200,
    node_separation: 120,
    edge_separation: 50,
    vertices: true,
};

/// Parameters for the display pass: the view's stored `automaticLayout`
/// (filtered → base), else [`DISPLAY_FALLBACK`], with the separations
/// raised to the [`DISPLAY_FALLBACK`] floors (100/50
/// reads as cramped against 350 px-wide boxes — the DSL `autoLayout`
/// statement stores exactly those defaults). Semantics follow graphviz,
/// where ranksep/nodesep are LOWER bounds: a file asking for more gets
/// more; the direction and vertex routing stay the file's. The dialog
/// path is untouched — an explicit dialog run is precise, file for file.
/// Display stays deterministic and file-driven: no byte of any
/// output changes (geometry is session state until a save seeds it).
#[must_use]
pub(crate) fn display_params(workspace: &Workspace, view_key: &str) -> AutomaticLayout {
    let mut params = stored_for(workspace, view_key).unwrap_or(DISPLAY_FALLBACK);
    params.rank_separation = params.rank_separation.max(DISPLAY_FALLBACK.rank_separation);
    params.node_separation = params.node_separation.max(DISPLAY_FALLBACK.node_separation);
    params
}

/// The display auto-layout for an un-laid-out view: `Some(engine
/// geometry)` when [`view_needs_layout`] holds and the bundled
/// [`Engine::Layered`] run succeeds, `None` when the view HAS stored
/// coordinates (it renders verbatim) or the engine run failed. The shell
/// merges this UNDER the user overlay ([`Overrides::display`]), so one
/// drag pins that element and leaves the engine's other placements.
#[must_use]
pub fn auto_display(workspace: &Workspace, view_key: &str, scene: &Scene) -> Option<Overrides> {
    if !view_needs_layout(workspace, view_key) {
        return None;
    }
    apply(scene, &display_params(workspace, view_key)).ok()
}

/// Seed `overlay` with the display geometry it rides on (the
/// SAVE-side twin of [`auto_display`]): a view that opened un-laid-out
/// displays engine geometry, so the user saw it — fill that geometry in
/// for every id the overlay does not cover so `apply_overlay` writes a
/// COMPLETE layout: the saved JSON then renders exactly like the screen
/// did before the save, instead of a single dragged box at real
/// coordinates and the rest piled at `0,0`. `None` = nothing to seed
/// (stored layout present, or the engine run failed).
#[must_use]
pub fn seed_overlay(
    workspace: &Workspace,
    view_key: &str,
    overlay: &crate::commands::LayoutOverlay,
) -> Option<crate::commands::LayoutOverlay> {
    use oxidizr_core::views::ColorScheme;

    let scene = crate::scene::build(workspace, view_key, ColorScheme::Light, None, None).ok()?;
    let auto = auto_display(workspace, view_key, &scene)?;
    let mut seeded = crate::commands::LayoutOverlay {
        view_key: view_key.to_owned(),
        elements: auto.elements,
        relationships: auto.relationships,
        dimensions: overlay.dimensions,
    };
    seeded.elements.extend(overlay.elements.clone());
    seeded.relationships.extend(overlay.relationships.clone());
    Some(seeded)
}

/// The layout graph of `scene`: one node per ITEM (id + the
/// style-resolved box size), one edge per relationship edge between
/// items (the scene already dropped boundary/unresolvable endpoints).
/// Content order = insertion order (deterministic).
#[must_use]
pub(crate) fn graph_for(scene: &Scene, config: LayoutConfig) -> LayoutGraph {
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

/// Run [`Engine::Layered`] over `scene` — the dialog default and the
/// headless-test entry point (module docs for the translation).
///
/// # Errors
///
/// As `apply_with`.
pub fn apply(scene: &Scene, layout: &AutomaticLayout) -> Result<Overrides, LayoutError> {
    apply_with(Engine::Layered, scene, layout)
}

/// Run `engine` over `scene` and translate the result into scene
/// [`Overrides`]: node placements become element top-lefts, route
/// interiors become relationship waypoints (module docs).
///
/// # Errors
///
/// Any [`LayoutError`] from graph validation or the engine run;
/// `Engine::Dot` with `$OXIDZR_DOT_BIN` unset reports
/// `LayoutError::DotBackend` rather than spawning nothing silently.
pub(crate) fn apply_with(
    engine: Engine,
    scene: &Scene,
    layout: &AutomaticLayout,
) -> Result<Overrides, LayoutError> {
    let graph = graph_for(scene, config_for(layout));
    let result = match engine {
        Engine::Layered => Layered::new().layout(&graph)?,
        #[cfg(feature = "dot-backend")]
        Engine::Dot => {
            let dot = Dot::from_env()
                .ok_or_else(|| LayoutError::DotBackend("OXIDZR_DOT_BIN is not set".to_owned()))?;
            dot.layout(&graph)?
        }
    };
    Ok(translate(&result))
}

/// Engine placements → scene overrides (module docs): endpoints of
/// every route are re-derived by the scene, so only the interior
/// points transfer.
fn translate(result: &LayoutResult) -> Overrides {
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
    overrides
}

/// The persisted form of the dialog parameters — eframe storage carries
/// PARAMETERS only (the produced geometry is session state):
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

    /// The `chain` fixture with every stored coordinate zeroed — the
    /// shape a DSL WITHOUT `location` statements loads as.
    fn stacked() -> Workspace {
        let mut ws = chain(None);
        for entry in &mut ws.views.system_landscape_views[0].content.elements {
            entry.x = 0;
            entry.y = 0;
        }
        ws
    }

    #[test]
    fn view_needs_layout_reads_rendered_coordinates() {
        assert!(!view_needs_layout(&chain(None), "L1"));
        assert!(view_needs_layout(&stacked(), "L1"));
        // A filtered view reads the RENDERED base's coordinates.
        let mut filtered = stacked();
        filtered
            .views
            .filtered_views
            .push(oxidizr_core::views::FilteredView {
                base: ViewBase::new("F1", 2),
                base_view_key: "L1".to_string(),
                mode: None,
                tags: Vec::new(),
            });
        assert!(view_needs_layout(&filtered, "F1"));
        assert!(!view_needs_layout(&chain(None), "nope"));
    }

    #[test]
    fn auto_display_layers_only_unlaid_out_views() {
        // Stored placements → render verbatim.
        let ws = chain(None);
        assert!(auto_display(&ws, "L1", &scene_of(&ws, None)).is_none());
        // All-zero stored geometry → the engine's exact placements AT
        // the display floors (rank 200 → box pitch 350 + 200).
        let ws = stacked();
        let auto = auto_display(&ws, "L1", &scene_of(&ws, None)).unwrap();
        assert_eq!(auto.elements.get(&id(1)), Some(&(0.0, 0.0)));
        assert_eq!(auto.elements.get(&id(2)), Some(&(550.0, 0.0)));
        assert_eq!(auto.elements.get(&id(3)), Some(&(1100.0, 0.0)));
    }

    #[test]
    fn display_params_floor_the_separations_only_upwards() {
        // Stored separations are LOWER bounds (graphviz
        // semantics) — a cramped `autoLayout` (100/50) rides up to the
        // floors, a generous one survives untouched, and the direction
        // + vertex routing always stay the file's.
        let cramped = chain(Some(AutomaticLayout::default()));
        let params = display_params(&cramped, "L1");
        assert_eq!(params.rank_separation, DISPLAY_FALLBACK.rank_separation);
        assert_eq!(params.node_separation, DISPLAY_FALLBACK.node_separation);
        let generous = chain(Some(AutomaticLayout {
            rank_direction: Some(RankDirection::TopBottom),
            rank_separation: 400,
            node_separation: 300,
            edge_separation: 90,
            vertices: false,
        }));
        let params = display_params(&generous, "L1");
        assert_eq!((params.rank_separation, params.node_separation), (400, 300));
        assert_eq!(params.rank_direction, Some(RankDirection::TopBottom));
        assert!(!params.vertices);
        // No stored layout at all → the full fallback.
        assert_eq!(display_params(&chain(None), "L1"), DISPLAY_FALLBACK);
    }

    #[test]
    fn display_overrides_pin_user_edits_on_top_of_the_engine() {
        let auto = Overrides {
            elements: [(id(1), (0.0, 0.0)), (id(2), (450.0, 0.0))]
                .into_iter()
                .collect(),
            relationships: HashMap::new(),
        };
        let user = Overrides {
            elements: [(id(2), (7.0, 7.0))].into_iter().collect(),
            relationships: HashMap::new(),
        };
        let merged = Overrides::display(&auto, Some(&user));
        assert_eq!(merged.elements.get(&id(1)), Some(&(0.0, 0.0)));
        assert_eq!(merged.elements.get(&id(2)), Some(&(7.0, 7.0)));
        assert_eq!(Overrides::display(&auto, None), auto);
    }

    #[test]
    fn seed_overlay_persists_the_displayed_geometry() {
        let mut overlay = crate::commands::LayoutOverlay::for_view("L1");
        overlay.elements.insert(id(1), (7.0, 7.0));
        let seeded = seed_overlay(&stacked(), "L1", &overlay).unwrap();
        assert_eq!(seeded.elements.get(&id(1)), Some(&(7.0, 7.0))); // the pin wins
        assert_eq!(seeded.elements.get(&id(2)), Some(&(550.0, 0.0))); // the rest rides the engine
        assert_eq!(seeded.elements.get(&id(3)), Some(&(1100.0, 0.0)));
        // A laid-out view has nothing to seed.
        let ws = chain(None);
        assert!(seed_overlay(&ws, "L1", &overlay).is_none());
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
            vec![(350.0, 75.0), (450.0, 75.0)],
            "facing side anchors from the moved boxes, no interior"
        );
    }

    #[test]
    // Exact layout values (integer separations, exact halves): the
    // axis-alignment check IS equality by design (the engine output is
    // deterministic).
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
    fn apply_with_layered_is_apply() {
        // The session-engine seam must not change the default
        // path at all — byte-identical overrides.
        let ws = chain(None);
        let scene = scene_of(&ws, None);
        let plain = apply(&scene, &AutomaticLayout::default()).unwrap();
        let layered = apply_with(Engine::Layered, &scene, &AutomaticLayout::default()).unwrap();
        assert_eq!(plain, layered);
    }

    #[test]
    fn layered_is_always_available_and_default() {
        assert_eq!(Engine::available().first(), Some(&Engine::Layered));
        assert_eq!(Engine::default(), Engine::Layered);
        assert_eq!(Engine::Layered.label(), "Layered");
    }

    #[cfg(feature = "dot-backend")]
    #[test]
    fn dot_without_the_env_is_an_error_value() {
        // Only the UNSET leg is testable in-process (the set leg needs a
        // real graphviz binary); a set var just skips this leg.
        if std::env::var_os("OXIDZR_DOT_BIN").is_some_and(|v| !v.is_empty()) {
            eprintln!("OXIDZR_DOT_BIN set — skipping unset-leg check");
            return;
        }
        let ws = chain(None);
        assert!(matches!(
            apply_with(
                Engine::Dot,
                &scene_of(&ws, None),
                &AutomaticLayout::default()
            ),
            Err(LayoutError::DotBackend(_))
        ));
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
