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
//! titles.

use epaint::{Color32, Rect as ERect, Shape, Stroke, pos2};

use crate::paint::Selection;
use crate::scene::{Rect, Scene};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Boundary, BoundaryKind, Item};
    use oxidizr_core::Id;
    use oxidizr_core::validate::ElementKind;
    use oxidizr_core::views::{IconPosition, LineStyle, Shape as ElementShape};

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
}
