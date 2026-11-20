//! The minimap + quick-nav core (M4-4, the reference `navigator.jspf`
//! analogue): an [`Overview`] maps the scene's WORLD bounds into a fixed
//! screen box and emits the whole box as an inert [`epaint::Shape`] list
//! — the same headless contract as [`crate::paint`] (Category A
//! testable, the shell just draws the list; the R-2 boundary guard holds
//! because `epaint` is geometry-only).
//!
//! The map is UNIFORM-scaled and centred (the reference navigator keeps
//! proportions too); [`Overview::map_to_world`] is the click/drag
//! inverse: a point picked in the map is the world point the shell
//! centers its viewport on. The current viewport appears as an accent
//! rect, clipped by the shell's painter to the box.
//!
//! Quick-nav rides the same module: [`nav_matches`] is the pure filter
//! the shell's search field runs over element names/ids and view
//! titles. So does the sidebar's kind split: [`section`] + [`grouped`]
//! partition the view list into per-type sections (§P3-M7).

use epaint::{Color32, Rect as ERect, Shape, Stroke, pos2};
use oxidizr_core::Id;

use crate::paint::Selection;
use crate::scene::{Rect, Scene};
use oxidizr_core::views::View;

/// Minimap box width in screen px (bottom-right of the canvas).
pub const MAP_WIDTH_PX: f32 = 220.0;
/// Minimap box height in screen px.
pub const MAP_HEIGHT_PX: f32 = 140.0;
/// Gap between the minimap box and the canvas edges.
pub const MAP_MARGIN_PX: f32 = 12.0;
/// Content padding INSIDE the box (the scaled scene never touches the
/// border stroke the shell draws).
pub const MAP_PAD_PX: f32 = 4.0;

/// Item boxes in the map (the reference navigator greys them; the
/// default element blue reads on both schemes).
pub const ITEM_FILL: Color32 = Color32::from_rgb(96, 144, 192);
/// Selected item boxes — the same accent as the main canvas.
pub const SELECTED_FILL: Color32 = crate::paint::SELECTION_COLOR;
/// Boundary outlines.
pub const BOUNDARY_STROKE: Color32 = Color32::from_gray(150);
/// The live-viewport rect (orange: distinguishable from the selection
/// accent on both schemes).
pub const VIEWPORT_STROKE: Color32 = Color32::from_rgb(220, 90, 40);

/// The world↔map transform of one minimap frame (world coords → the
/// box's screen px, uniform scale, content centred). Built per frame
/// from `Scene::bounds` + the box rect — stateless like [`crate::paint`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Overview {
    bounds: Rect,
    map: ERect,
    scale: f64,
    offset: (f64, f64),
}

impl Overview {
    /// Fit `bounds` into `map` with [`MAP_PAD_PX`] padding. `None` when
    /// either rect is degenerate or non-finite (an empty scene's zero
    /// rect, or a box too small to paint) — the shell then skips the
    /// minimap entirely.
    #[must_use]
    pub fn new(bounds: Rect, map: ERect) -> Option<Self> {
        if bounds.width <= 0.0
            || bounds.height <= 0.0
            || !bounds.width.is_finite()
            || !bounds.height.is_finite()
            || !map.is_positive()
        {
            return None;
        }
        let pad = f64::from(MAP_PAD_PX);
        let inner_w = f64::from(map.width()) - 2.0 * pad;
        let inner_h = f64::from(map.height()) - 2.0 * pad;
        if inner_w <= 0.0 || inner_h <= 0.0 {
            return None;
        }
        let scale = (inner_w / bounds.width).min(inner_h / bounds.height);
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }
        let center = bounds.center();
        let map_center = (f64::from(map.center().x), f64::from(map.center().y));
        Some(Self {
            bounds,
            map,
            scale,
            offset: (
                map_center.0 - center.0 * scale,
                map_center.1 - center.1 * scale,
            ),
        })
    }

    /// The world bounds this map fits (for shell-side triage).
    #[must_use]
    pub const fn bounds(&self) -> Rect {
        self.bounds
    }

    /// The screen box the map paints into.
    #[must_use]
    pub const fn map(&self) -> ERect {
        self.map
    }

    /// World → map screen px.
    #[must_use]
    pub fn world_to_map(self, world: (f64, f64)) -> (f64, f64) {
        (
            world.0 * self.scale + self.offset.0,
            world.1 * self.scale + self.offset.1,
        )
    }

    /// Map screen px → world (the exact inverse of
    /// [`Self::world_to_map`] up to float rounding).
    #[must_use]
    pub fn map_to_world(self, map: (f64, f64)) -> (f64, f64) {
        (
            (map.0 - self.offset.0) / self.scale,
            (map.1 - self.offset.1) / self.scale,
        )
    }

    /// Paint the box contents: boundaries (outline), items (fill, accent
    /// when selected) and the live-viewport `view_world` rect on top.
    /// No background — the shell draws the frame under the shapes.
    #[must_use]
    pub fn shapes(
        &self,
        scene: &Scene,
        view_world: Option<Rect>,
        selection: Option<&Selection>,
    ) -> Vec<Shape> {
        let mut out = Vec::with_capacity(scene.boundaries.len() + scene.items.len() + 1);
        for boundary in &scene.boundaries {
            out.push(Shape::rect_stroke(
                self.map_rect(boundary.rect),
                0.0,
                Stroke::new(1.0, BOUNDARY_STROKE),
                epaint::StrokeKind::Inside,
            ));
        }
        for item in &scene.items {
            let selected =
                matches!(selection, Some(Selection::Items(ids)) if ids.contains(&item.id));
            out.push(Shape::rect_filled(
                self.map_rect(item.rect),
                0.0,
                if selected { SELECTED_FILL } else { ITEM_FILL },
            ));
        }
        if let Some(view) = view_world {
            out.push(Shape::rect_stroke(
                self.map_rect(view),
                0.0,
                Stroke::new(1.5, VIEWPORT_STROKE),
                epaint::StrokeKind::Inside,
            ));
        }
        out
    }

    /// One world rect into the map (corners projected — a uniform scale
    /// keeps it axis-aligned).
    #[allow(clippy::cast_possible_truncation)] // map px fit f32 (screen space)
    fn map_rect(&self, world: Rect) -> ERect {
        let min = self.world_to_map((world.x, world.y));
        let max = self.world_to_map((world.x + world.width, world.y + world.height));
        ERect::from_min_max(
            pos2(min.0 as f32, min.1 as f32),
            pos2(max.0 as f32, max.1 as f32),
        )
    }
}

/// Quick-nav filter (M4-4): case-insensitive substring of `query` in
/// either the display `name` or the `id`. An empty/whitespace query
/// matches NOTHING — the result list only opens once the user types.
#[must_use]
pub fn nav_matches(query: &str, name: &str, id: &str) -> bool {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return false;
    }
    name.to_lowercase().contains(&needle) || id.to_lowercase().contains(&needle)
}

/// The sidebar section of one view kind (§P3-M7, user request: split
/// the view list by view type): `(rank, header)`. The order is C4
/// granularity first (landscape → context → container → component),
/// then deployment and dynamic, then the derived kinds (filtered,
/// image) and custom last.
#[must_use]
pub const fn section(view: View<'_>) -> (u8, &'static str) {
    match view {
        View::Landscape(_) => (0, "System landscape"),
        View::Context(_) => (1, "System context"),
        View::Container(_) => (2, "Container"),
        View::Component(_) => (3, "Component"),
        View::Deployment(_) => (4, "Deployment"),
        View::Dynamic(_) => (5, "Dynamic"),
        View::Filtered(_) => (6, "Filtered"),
        View::Image(_) => (7, "Image"),
        View::Custom(_) => (8, "Custom"),
    }
}

/// Group `views` by kind for the sidebar (§P3-M7): `(header, members)`
/// runs in [`section`] order, empty kinds skipped. Inside a section the
/// input's (order, key) ranking survives — each block keeps the file's
/// own view order (`sort_by_key` is STABLE and the key carries the
/// ranking).
#[must_use]
pub fn grouped<'a>(views: &[View<'a>]) -> Vec<(&'static str, Vec<View<'a>>)> {
    let mut sorted = views.to_vec();
    sorted.sort_by_key(|view| (section(*view).0, view.base().order, view.key().to_owned()));
    let mut runs: Vec<(&'static str, Vec<View<'_>>)> = Vec::new();
    for view in sorted {
        let header = section(view).1;
        match runs.last_mut() {
            Some((last, members)) if *last == header => members.push(view),
            _ => runs.push((header, vec![view])),
        }
    }
    runs
}

/// Factual rows about the current selection (§P3-M11 M11-11, the right
/// panel's Info block): labelled key/value lines for one element, the
/// element group, or one edge. Edge endpoints resolve to ELEMENT NAMES
/// (ids only when an end is off-scene); rows that would be empty are
/// skipped, so the block stays compact for sparse models.
#[must_use]
pub fn selection_info(scene: &Scene, selection: &Selection) -> Vec<(&'static str, String)> {
    let name_of = |id: &Id| {
        scene
            .items
            .iter()
            .find(|item| item.id == *id)
            .map_or_else(|| id.to_string(), |item| item.name.clone())
    };
    let mut rows: Vec<(&'static str, String)> = Vec::new();
    match selection {
        Selection::Items(ids) => {
            if let [only] = ids.as_slice() {
                let Some(item) = scene.items.iter().find(|item| item.id == *only) else {
                    return rows;
                };
                rows.push(("Name", item.name.clone()));
                rows.push(("Id", item.id.to_string()));
                rows.push(("Kind", format!("{:?}", item.kind)));
                if let Some(group) = &item.group {
                    rows.push(("Group", group.clone()));
                }
                if let Some(description) = &item.description {
                    rows.push(("Description", description.clone()));
                }
                if let Some(metadata) = &item.metadata {
                    rows.push(("Metadata", metadata.clone()));
                }
                rows.push((
                    "Location",
                    format!(
                        "({}, {}) · {} × {}",
                        item.rect.x.round(),
                        item.rect.y.round(),
                        item.rect.width.round(),
                        item.rect.height.round()
                    ),
                ));
                if let Some(url) = &item.url {
                    rows.push(("Url", url.clone()));
                }
            } else if !ids.is_empty() {
                rows.push(("Elements", ids.len().to_string()));
                let mut names: Vec<String> = ids.iter().take(8).map(&name_of).collect();
                if ids.len() > 8 {
                    names.push("…".to_string());
                }
                rows.push(("Contains", names.join(", ")));
            }
        }
        Selection::Edge(id) => {
            let Some(edge) = scene.edges.iter().find(|edge| edge.id == *id) else {
                return rows;
            };
            rows.push((
                "Label",
                edge.label.clone().unwrap_or_else(|| "(none)".to_string()),
            ));
            rows.push(("From", name_of(&edge.source)));
            rows.push(("To", name_of(&edge.destination)));
            if let Some(metadata) = &edge.metadata {
                rows.push(("Technology", metadata.clone()));
            }
            if let Some(order) = &edge.order {
                rows.push(("Order", order.clone()));
            }
            rows.push(("Waypoints", edge.points.len().saturating_sub(2).to_string()));
            if let Some(url) = &edge.url {
                rows.push(("Url", url.clone()));
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Boundary, BoundaryKind, Edge, Item};
    use oxidizr_core::Id;
    use oxidizr_core::validate::ElementKind;
    use oxidizr_core::views::{
        ComponentView, ContainerView, CustomView, DeploymentView, DynamicView, FilteredView,
        IconPosition, ImageView, LineStyle, Shape as ElementShape, SystemContextView,
        SystemLandscapeView, ViewBase, ViewContent,
    };

    /// Positional scene-rect constructor (keeps fixtures readable).
    fn wr(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    fn item(id: &str, x: f64, y: f64) -> Item {
        Item {
            id: Id::new(id),
            kind: ElementKind::Person,
            name: format!("Box {id}"),
            description: None,
            metadata: None,
            group: None,
            url: None,
            rect: wr(x, y, 350.0, 150.0),
            shape: ElementShape::Box,
            background: "#8db4dc".to_string(),
            stroke: "#1a1a1a".to_string(),
            stroke_width: 2,
            border: LineStyle::Solid,
            color: "#000000".to_string(),
            font_size: 24,
            opacity: 100,
            icon: None,
            icon_position: IconPosition::Top,
        }
    }

    fn scene() -> Scene {
        Scene {
            view_key: "v".to_string(),
            title: "V".to_string(),
            background: "#ffffff".to_string(),
            boundaries: vec![Boundary {
                kind: BoundaryKind::Group,
                id: "group:g".to_string(),
                label: "g".to_string(),
                metadata: None,
                rect: wr(0.0, 0.0, 1000.0, 600.0),
                background: "#fafafa".to_string(),
                stroke: "#c0c0c0".to_string(),
                stroke_width: 2,
                border: LineStyle::Solid,
                color: "#000000".to_string(),
                font_size: 24,
                opacity: 100,
            }],
            items: vec![item("1", 50.0, 50.0), item("2", 500.0, 300.0)],
            edges: Vec::new(),
            bounds: wr(0.0, 0.0, 1000.0, 600.0),
        }
    }

    fn edge(points: Vec<(f64, f64)>) -> Edge {
        Edge {
            id: Id::new("e"),
            source: Id::new("1"),
            destination: Id::new("2"),
            points,
            label: None,
            metadata: Some("[HTTPS]".to_string()),
            order: None,
            color: "#000000".to_string(),
            thickness: 2,
            line_style: LineStyle::Solid,
            font_size: 24,
            label_width: 200,
            label_position: 50,
            opacity: 100,
            url: None,
        }
    }

    #[test]
    fn selection_info_reports_item_group_and_edge() {
        let mut s = scene();
        s.items[0].description = Some("Pays money".to_string());
        s.edges.push(edge(vec![(1.0, 1.0), (2.0, 2.0), (3.0, 3.0)]));
        let get = |rows: &[(&'static str, String)], key| {
            rows.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone())
        };
        // A single element: the full identity + geometry block.
        let rows = selection_info(&s, &Selection::Items(vec![Id::new("1")]));
        assert_eq!(get(&rows, "Name").as_deref(), Some("Box 1"));
        assert_eq!(get(&rows, "Kind").as_deref(), Some("Person"));
        assert_eq!(get(&rows, "Description").as_deref(), Some("Pays money"));
        assert_eq!(
            get(&rows, "Location").as_deref(),
            Some("(50, 50) · 350 × 150")
        );
        assert_eq!(get(&rows, "Url"), None, "absent rows are skipped");
        // A group: count + names.
        let rows = selection_info(&s, &Selection::Items(vec![Id::new("1"), Id::new("2")]));
        assert_eq!(get(&rows, "Elements").as_deref(), Some("2"));
        assert_eq!(get(&rows, "Contains").as_deref(), Some("Box 1, Box 2"));
        // An edge: endpoint NAMES, technology, waypoint count.
        let rows = selection_info(&s, &Selection::Edge(Id::new("e")));
        assert_eq!(get(&rows, "Label").as_deref(), Some("(none)"));
        assert_eq!(get(&rows, "From").as_deref(), Some("Box 1"));
        assert_eq!(get(&rows, "To").as_deref(), Some("Box 2"));
        assert_eq!(get(&rows, "Technology").as_deref(), Some("[HTTPS]"));
        assert_eq!(get(&rows, "Waypoints").as_deref(), Some("1"));
        // Selections pointing off-scene render nothing.
        assert_eq!(selection_info(&s, &Selection::Items(vec![])), Vec::new());
        assert_eq!(
            selection_info(&s, &Selection::Edge(Id::new("gone"))),
            Vec::new()
        );
    }

    fn box_rect() -> ERect {
        ERect::from_min_max(pos2(0.0, 0.0), pos2(220.0, 140.0))
    }

    #[test]
    fn degenerate_inputs_build_no_map() {
        let map = box_rect();
        assert!(
            Overview::new(wr(0.0, 0.0, 0.0, 0.0), map).is_none(),
            "an empty scene's zero rect has no map"
        );
        assert!(
            Overview::new(
                wr(0.0, 0.0, 1000.0, 600.0),
                ERect::from_min_max(pos2(0.0, 0.0), pos2(4.0, 1.0))
            )
            .is_none(),
            "a box padded to nothing has no map"
        );
    }

    #[test]
    fn transform_fits_centers_and_inverts() {
        let bounds = wr(100.0, 100.0, 1000.0, 600.0);
        let ov = Overview::new(bounds, box_rect()).expect("map");
        // Uniform fit: 1000x600 world into 212x132 inner px ⇒ width binds.
        let inner = f64::from(220.0 - 2.0 * MAP_PAD_PX);
        assert!((ov.scale - inner / 1000.0).abs() < 1e-9);
        // Content centred: the bounds centre maps to the box centre.
        let center = ov.world_to_map(bounds.center());
        assert!((center.0 - 110.0).abs() < 1e-6 && (center.1 - 70.0).abs() < 1e-6);
        // Invertible: map_to_world(world_to_map(p)) == p.
        let world = (375.0, 425.0);
        let back = ov.map_to_world(ov.world_to_map(world));
        assert!((back.0 - world.0).abs() < 1e-6 && (back.1 - world.1).abs() < 1e-6);
    }

    #[test]
    fn shapes_paint_boundaries_items_and_viewport() {
        let scene = scene();
        let ov = Overview::new(scene.bounds, box_rect()).expect("map");
        let plain = ov.shapes(&scene, None, None);
        assert_eq!(plain.len(), 1 + 2, "one boundary outline + two items");
        let with_view = ov.shapes(&scene, Some(wr(0.0, 0.0, 500.0, 300.0)), None);
        assert_eq!(with_view.len(), plain.len() + 1, "viewport rect appended");
        // The appended rect carries the viewport stroke color.
        let Shape::Rect(rect) = with_view.last().expect("viewport rect") else {
            panic!("expected a rect shape");
        };
        assert_eq!(rect.stroke.color, VIEWPORT_STROKE);
        // Selection recolors exactly the picked item's fill.
        let selected = ov.shapes(&scene, None, Some(&Selection::Items(vec![Id::new("2")])));
        let fills: Vec<Color32> = selected
            .iter()
            .filter_map(|s| match s {
                Shape::Rect(r) if r.fill != Color32::TRANSPARENT => Some(r.fill),
                _ => None,
            })
            .collect();
        assert_eq!(
            fills,
            vec![ITEM_FILL, SELECTED_FILL],
            "item 2 painted with the accent fill"
        );
    }

    #[test]
    fn nav_filter_is_case_insensitive_over_name_and_id() {
        assert!(nav_matches("ali", "Alice", "1"));
        assert!(nav_matches(" 2 ", "Alice", "x2y"), "id hit, trimmed");
        assert!(nav_matches("BOB", "bob", "9"), "case-insensitive");
        assert!(
            !nav_matches("", "Alice", "1"),
            "empty query matches nothing"
        );
        assert!(!nav_matches("   ", "Alice", "1"));
        assert!(!nav_matches("zed", "Alice", "1"));
    }

    #[test]
    fn grouped_splits_the_view_list_by_kind() {
        // Sections in canonical order; inside a section the (order, key)
        // ranking survives, not the (scrambled) input order.
        let kinds = NineKinds::new();
        let shape: Vec<(&str, Vec<&str>)> = grouped(&kinds.scrambled())
            .iter()
            .map(|(header, members)| (*header, members.iter().map(|view| view.key()).collect()))
            .collect();
        assert_eq!(
            shape,
            vec![
                ("System landscape", vec!["L1", "L2"]),
                ("System context", vec!["C1"]),
                ("Container", vec!["CT1"]),
                ("Component", vec!["P1"]),
                ("Deployment", vec!["D1"]),
                ("Dynamic", vec!["Y1"]),
                ("Filtered", vec!["F1"]),
                ("Image", vec!["I1"]),
                ("Custom", vec!["X1"]),
            ]
        );
        assert!(grouped(&[]).is_empty());
    }

    /// Exhaustive §P3-M7 fixture: one view per `View` variant, plus TWO
    /// landscape views to pin the (order, key) ranking inside a section.
    struct NineKinds {
        land_a: SystemLandscapeView,
        land_b: SystemLandscapeView,
        ctx: SystemContextView,
        cont: ContainerView,
        comp: ComponentView,
        dep: DeploymentView,
        dynv: DynamicView,
        flt: FilteredView,
        img: ImageView,
        custom: CustomView,
    }

    impl NineKinds {
        fn new() -> Self {
            let content = ViewContent::default;
            Self {
                land_a: SystemLandscapeView {
                    base: ViewBase::new("L2", 2),
                    content: content(),
                    enterprise_boundary_visible: false,
                    animations: Vec::new(),
                },
                land_b: SystemLandscapeView {
                    base: ViewBase::new("L1", 1),
                    content: content(),
                    enterprise_boundary_visible: false,
                    animations: Vec::new(),
                },
                ctx: SystemContextView {
                    base: ViewBase::new("C1", 9),
                    content: content(),
                    software_system_id: Id::new("1"),
                    enterprise_boundary_visible: false,
                    animations: Vec::new(),
                },
                cont: ContainerView {
                    base: ViewBase::new("CT1", 9),
                    content: content(),
                    software_system_id: Id::new("1"),
                    external_software_system_boundaries_visible: false,
                    animations: Vec::new(),
                },
                comp: ComponentView {
                    base: ViewBase::new("P1", 9),
                    content: content(),
                    container_id: Id::new("2"),
                    external_container_boundaries_visible: false,
                    animations: Vec::new(),
                },
                dep: DeploymentView {
                    base: ViewBase::new("D1", 9),
                    content: content(),
                    environment: "Default".to_owned(),
                    software_system_id: None,
                    animations: Vec::new(),
                },
                dynv: DynamicView {
                    base: ViewBase::new("Y1", 9),
                    content: content(),
                    element_id: None,
                    deployment_environment: None,
                    external_boundaries_visible: false,
                },
                flt: FilteredView {
                    base: ViewBase::new("F1", 9),
                    base_view_key: "L1".to_owned(),
                    mode: None,
                    tags: Vec::new(),
                },
                img: ImageView {
                    base: ViewBase::new("I1", 9),
                    element_id: None,
                    content: None,
                    content_light: None,
                    content_dark: None,
                    content_type: None,
                },
                custom: CustomView {
                    base: ViewBase::new("X1", 9),
                    content: content(),
                    animations: Vec::new(),
                    software_system_id: None,
                },
            }
        }

        /// Input with scrambled ranks: filtered/image/custom lead and
        /// `land_a` (rank 2) precedes `land_b` (rank 1).
        fn scrambled(&self) -> Vec<View<'_>> {
            vec![
                View::Filtered(&self.flt),
                View::Landscape(&self.land_a),
                View::Image(&self.img),
                View::Landscape(&self.land_b),
                View::Context(&self.ctx),
                View::Container(&self.cont),
                View::Component(&self.comp),
                View::Deployment(&self.dep),
                View::Dynamic(&self.dynv),
                View::Custom(&self.custom),
            ]
        }
    }
}
