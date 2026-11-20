//! Headless viewport transform + hit-testing (P3-D3): world (stored
//! layout) coordinates ↔ screen pixels, AABB item picks and
//! point-to-polyline edge picks over a [`Scene`] — hand-rolled, no
//! geometry crate, no egui (the boundary guard keeps it that way).
//!
//! The [`Viewport`] is the shell's only view state: a uniform `scale`
//! plus a pixel `offset`, `screen = world * scale + offset`. The zoom
//! clamps and the pick tolerance are OURS (the reference inherits its
//! limits from the jsPlumb canvas — visual parity is out of contract,
//! P3-D0); they are stable constants the M1-4 shell and its tests rely
//! on.
//!
//! Pick semantics (M1):
//!
//! - Items paint in content order, so the LAST box containing the point
//!   is on top and wins ([`pick`] scans [`Scene::items`] back to front).
//! - Boundaries are not pickable yet — nothing selects or drags them
//!   until M3 layout editing.
//! - Edges hit by polyline distance: the tolerance is
//!   [`HIT_TOLERANCE_PX`] screen pixels converted to world units at the
//!   current scale, so thin lines stay clickable when zoomed out.
//! - Item over edge wins: the reference draws arrows under boxes, so a
//!   point inside a box is never an arrow click.

use crate::scene::{Rect, Scene};

/// Smallest zoom: the whole-landscape overview floor (ours — see module
/// docs).
pub const MIN_SCALE: f64 = 0.05;
/// Largest zoom: the pixel-detail ceiling (ours).
pub const MAX_SCALE: f64 = 20.0;
/// Pick tolerance in SCREEN pixels ([`pick`]); divided by the scale to
/// get the world-space polyline distance.
pub const HIT_TOLERANCE_PX: f64 = 6.0;
/// Minimum screen padding [`Viewport::fit`] keeps around the scene
/// bounds.
pub const FIT_PADDING_PX: f64 = 40.0;

/// The world→screen similarity transform: `screen = world * scale +
/// offset` ([`Viewport::world_to_screen`]). Scale is always within
/// [`MIN_SCALE`]..=`MAX_SCALE` (constructed through [`Viewport::new`] /
/// [`Viewport::fit`], never field-wise — the fields are private).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    scale: f64,
    offset: (f64, f64),
}

impl Viewport {
    /// A viewport at `scale` (clamped into [`MIN_SCALE`]..=`MAX_SCALE`,
    /// a NaN scale falling back to 1.0) with the world origin landing at
    /// `offset` screen pixels.
    #[must_use]
    pub fn new(scale: f64, offset: (f64, f64)) -> Self {
        Self {
            scale: clamped(scale, 1.0),
            offset,
        }
    }

    /// A viewport centring `bounds` in a `screen`-sized surface, zoomed
    /// to fit with [`FIT_PADDING_PX`] padding. Degenerate (zero-width or
    /// zero-height) bounds fit at scale 1.0, still centred.
    #[must_use]
    pub fn fit(bounds: Rect, screen: (f64, f64)) -> Self {
        let width = screen.0.max(1.0);
        let height = screen.1.max(1.0);
        let scale = if bounds.width <= 0.0 || bounds.height <= 0.0 {
            1.0
        } else {
            let fit_width = (width - 2.0 * FIT_PADDING_PX) / bounds.width;
            let fit_height = (height - 2.0 * FIT_PADDING_PX) / bounds.height;
            fit_width.min(fit_height).clamp(MIN_SCALE, MAX_SCALE)
        };
        let center = bounds.center();
        Self {
            scale,
            offset: (
                width / 2.0 - center.0 * scale,
                height / 2.0 - center.1 * scale,
            ),
        }
    }

    /// The current zoom factor.
    #[must_use]
    pub const fn scale(self) -> f64 {
        self.scale
    }

    /// The screen pixel the world origin maps to.
    #[must_use]
    pub const fn offset(self) -> (f64, f64) {
        self.offset
    }

    /// World coordinates → screen pixels.
    #[must_use]
    pub fn world_to_screen(self, world: (f64, f64)) -> (f64, f64) {
        (
            world.0 * self.scale + self.offset.0,
            world.1 * self.scale + self.offset.1,
        )
    }

    /// Screen pixels → world coordinates (the exact inverse of
    /// [`Viewport::world_to_screen`] up to float rounding).
    #[must_use]
    pub fn screen_to_world(self, screen: (f64, f64)) -> (f64, f64) {
        (
            (screen.0 - self.offset.0) / self.scale,
            (screen.1 - self.offset.1) / self.scale,
        )
    }

    /// Multiply the scale by `factor` (clamped, NaN keeping the current
    /// scale) keeping the world point under screen `pivot` FIXED — the
    /// cursor-anchored zoom. At the clamp limits nothing moves.
    pub fn zoom_at(&mut self, factor: f64, pivot: (f64, f64)) {
        let target = clamped(self.scale * factor, self.scale);
        let ratio = target / self.scale;
        self.offset = (
            pivot.0 - (pivot.0 - self.offset.0) * ratio,
            pivot.1 - (pivot.1 - self.offset.1) * ratio,
        );
        self.scale = target;
    }

    /// Drag the canvas by a screen-pixel delta.
    pub fn pan_by(&mut self, delta: (f64, f64)) {
        self.offset = (self.offset.0 + delta.0, self.offset.1 + delta.1);
    }
}

/// Zoom clamped into [`MIN_SCALE`]..=`MAX_SCALE`; a NaN scale becomes
/// `fallback` (`f64::clamp` passes NaN through, which would poison every
/// later transform).
fn clamped(scale: f64, fallback: f64) -> f64 {
    if scale.is_nan() {
        fallback
    } else {
        scale.clamp(MIN_SCALE, MAX_SCALE)
    }
}

/// What a [`pick`] found: the index into the [`Scene`] layer (painting
/// order == vector order, so the index is also the z-slot).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// An item box — index into [`Scene::items`](crate::scene::Scene::items).
    Item(usize),
    /// An edge polyline — index into [`Scene::edges`](crate::scene::Scene::edges).
    Edge(usize),
}

/// Pick the top-most scene entry at screen pixel `point` under
/// `viewport`: items first (content order, last painted wins), then
/// edges within [`HIT_TOLERANCE_PX`]. `None` = canvas background.
#[must_use]
pub fn pick(scene: &Scene, viewport: Viewport, point: (f64, f64)) -> Option<Hit> {
    let world = viewport.screen_to_world(point);
    // Items paint in content order: the LAST box containing the point is
    // on top (reference: boxes sit above arrows).
    if let Some(index) = scene
        .items
        .iter()
        .rposition(|item| item.rect.contains(world))
    {
        return Some(Hit::Item(index));
    }
    let tolerance = HIT_TOLERANCE_PX / viewport.scale();
    scene
        .edges
        .iter()
        .position(|edge| distance_to_polyline(world, &edge.points) <= tolerance)
        .map(Hit::Edge)
}

/// Lasso helper (M3-3 box-select): the indices of every item whose box
/// INTERSECTS the world-space `rect`, in content order. Intersection,
/// not containment — a lasso clipping a box edge selects it, like the
/// reference's rubber-band drag.
#[must_use]
pub fn items_in_rect(scene: &Scene, rect: Rect) -> Vec<usize> {
    scene
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            item.rect.x < rect.x + rect.width
                && rect.x < item.rect.x + item.rect.width
                && item.rect.y < rect.y + rect.height
                && rect.y < item.rect.y + item.rect.height
        })
        .map(|(index, _)| index)
        .collect()
}

/// Index of the INTERIOR vertex of `points` nearest to `point`, when it
/// sits within `tol` world units — the waypoint-handle hit (P3-M4).
/// The polyline is the scene contract `[source, interiors…, destination]`,
/// so the endpoints are skipped: they belong to the boxes, never the
/// route. `None` for polylines without interior vertices.
#[must_use]
pub fn nearest_waypoint(point: (f64, f64), points: &[(f64, f64)], tol: f64) -> Option<usize> {
    points
        .iter()
        .enumerate()
        .skip(1)
        .take(points.len().saturating_sub(2))
        .filter(|(_, at)| distance(point, **at) <= tol)
        .min_by(|(_, a), (_, b)| distance(point, **a).total_cmp(&distance(point, **b)))
        .map(|(index, _)| index)
}

/// Where a new vertex splices into `points`: the start of the segment
/// nearest to `point` (a straight two-point edge inserts at 1; fewer
/// than two points have nothing to split — the append index comes back).
#[must_use]
pub fn insert_waypoint(point: (f64, f64), points: &[(f64, f64)]) -> usize {
    points
        .windows(2)
        .enumerate()
        .map(|(index, segment)| (index, distance_to_segment(point, segment[0], segment[1])))
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map_or(points.len(), |(index, _)| index + 1)
}

/// Distance from `point` to a polyline: `INFINITY` for the empty line,
/// point distance for a single vertex, else the minimum over the
/// segments ([`distance_to_segment`]).
#[must_use]
pub fn distance_to_polyline(point: (f64, f64), points: &[(f64, f64)]) -> f64 {
    match points.len() {
        0 => f64::INFINITY,
        1 => distance(point, points[0]),
        _ => points
            .windows(2)
            .map(|segment| distance_to_segment(point, segment[0], segment[1]))
            .fold(f64::INFINITY, f64::min),
    }
}

/// Distance from `point` to the segment `from`→`to` (clamped projection;
/// a zero-length segment degenerates to point distance — stored
/// duplicate vertices are legal JSON).
#[must_use]
pub fn distance_to_segment(point: (f64, f64), from: (f64, f64), to: (f64, f64)) -> f64 {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let length_sq = dx.mul_add(dx, dy * dy);
    if length_sq <= f64::EPSILON {
        return distance(point, from);
    }
    let t = (((point.0 - from.0) * dx + (point.1 - from.1) * dy) / length_sq).clamp(0.0, 1.0);
    distance(point, (from.0 + t * dx, from.1 + t * dy))
}

/// Euclidean point-point distance.
fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    dx.mul_add(dx, dy * dy).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Boundary, Edge, Item};
    use oxidizr_core::Id;
    use oxidizr_core::validate::ElementKind;
    use oxidizr_core::views::{IconPosition, LineStyle, Shape};

    fn rect(x: f64, y: f64) -> Rect {
        Rect {
            x,
            y,
            width: 100.0,
            height: 50.0,
        }
    }

    fn item(x: f64, y: f64) -> Item {
        Item {
            id: Id::new("1"),
            kind: ElementKind::Person,
            name: String::new(),
            description: None,
            metadata: None,
            group: None,
            url: None,
            rect: rect(x, y),
            shape: Shape::Box,
            background: "#ffffff".to_string(),
            stroke: "#000000".to_string(),
            stroke_width: 2,
            border: LineStyle::Solid,
            color: "#000000".to_string(),
            font_size: 24,
            opacity: 100,
            icon: None,
            icon_position: IconPosition::Top,
        }
    }

    fn edge(points: Vec<(f64, f64)>) -> Edge {
        Edge {
            id: Id::new("e"),
            source: Id::new("s"),
            destination: Id::new("d"),
            points,
            label: None,
            metadata: None,
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

    fn boundary(x: f64, y: f64, width: f64, height: f64) -> Boundary {
        Boundary {
            kind: crate::scene::BoundaryKind::Group,
            id: "group:root/Team".to_string(),
            label: "Team".to_string(),
            metadata: None,
            rect: Rect {
                x,
                y,
                width,
                height,
            },
            background: "#ffffff".to_string(),
            stroke: "#000000".to_string(),
            stroke_width: 2,
            border: LineStyle::Dashed,
            color: "#000000".to_string(),
            font_size: 24,
            opacity: 100,
        }
    }

    fn scene(items: Vec<Item>, edges: Vec<Edge>, boundaries: Vec<Boundary>) -> Scene {
        Scene {
            view_key: "K".to_string(),
            title: String::new(),
            background: "#ffffff".to_string(),
            boundaries,
            items,
            edges,
            bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
        }
    }

    #[test]
    fn viewport_clamps_zoom_and_round_trips() {
        assert_eq!(Viewport::new(0.001, (0.0, 0.0)).scale(), MIN_SCALE);
        assert_eq!(Viewport::new(1000.0, (0.0, 0.0)).scale(), MAX_SCALE);
        // NaN cannot poison the transform: a NaN scale falls back to 1.0.
        assert_eq!(Viewport::new(f64::NAN, (0.0, 0.0)).scale(), 1.0);
        let v = Viewport::new(2.0, (10.0, 20.0));
        let screen = v.world_to_screen((7.0, 9.0));
        assert_eq!(screen, (24.0, 38.0));
        assert_eq!(v.screen_to_world(screen), (7.0, 9.0));
    }

    #[test]
    fn zoom_at_keeps_pivot_world_point_fixed_and_clamps() {
        let mut v = Viewport::new(1.0, (0.0, 0.0));
        let pivot = (100.0, 50.0);
        v.zoom_at(1.25, pivot);
        assert_eq!(v.scale(), 1.25);
        assert_eq!(v.offset(), (-25.0, -12.5));
        assert_eq!(v.screen_to_world(pivot), (100.0, 50.0));
        // Past the ceiling: scale lands exactly on MAX_SCALE, pivot world
        // point unmoved.
        v.zoom_at(1.0e6, pivot);
        assert_eq!(v.scale(), MAX_SCALE);
        assert_eq!(v.screen_to_world(pivot), (100.0, 50.0));
        // Already clamped: ratio 1, nothing moves at all.
        let before = v;
        v.zoom_at(3.0, pivot);
        assert_eq!(v, before);
        // And down through the floor: scale lands exactly on MIN_SCALE;
        // the pivot world point survives within float noise (0.05 is not
        // a binary fraction).
        v.zoom_at(1.0e-6, pivot);
        assert_eq!(v.scale(), MIN_SCALE);
        let world = v.screen_to_world(pivot);
        assert!(
            (world.0 - 100.0).abs() < 1e-9 && (world.1 - 50.0).abs() < 1e-9,
            "pivot drifted to {world:?}"
        );
    }

    #[test]
    fn pan_by_moves_the_offset() {
        let mut v = Viewport::new(1.0, (10.0, 10.0));
        v.pan_by((5.0, -4.0));
        assert_eq!(v.offset(), (15.0, 6.0));
    }

    #[test]
    fn fit_centres_and_scales_scene_bounds() {
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 500.0,
        };
        let screen = (800.0, 600.0);
        let v = Viewport::fit(bounds, screen);
        // Width-limited fit with padding; recomputed the same way → exact.
        let expected_scale = (800.0 - 2.0 * FIT_PADDING_PX) / 1000.0;
        assert_eq!(v.scale(), expected_scale);
        let top_left = v.world_to_screen((bounds.x, bounds.y));
        let bottom_right = v.world_to_screen((1000.0, 500.0));
        assert_eq!(top_left.0, FIT_PADDING_PX);
        assert_eq!(bottom_right.0, 800.0 - FIT_PADDING_PX);
        assert!(top_left.1 > FIT_PADDING_PX && bottom_right.1 < 600.0 - FIT_PADDING_PX);
        // Centred: equal vertical margins.
        assert_eq!(
            top_left.1 - FIT_PADDING_PX,
            600.0 - FIT_PADDING_PX - bottom_right.1
        );
        // Degenerate bounds: scale 1.0, still centred.
        let point_bounds = Rect {
            x: 40.0,
            y: 60.0,
            width: 0.0,
            height: 0.0,
        };
        let v = Viewport::fit(point_bounds, screen);
        assert_eq!(v.scale(), 1.0);
        assert_eq!(v.world_to_screen((40.0, 60.0)), (400.0, 300.0));
    }

    #[test]
    fn pick_topmost_item_wins_and_gaps_miss() {
        let mut below = item(0.0, 0.0);
        below.id = Id::new("below");
        let mut above = item(50.0, 25.0);
        above.id = Id::new("above");
        let s = scene(vec![below, above], Vec::new(), Vec::new());
        let v = Viewport::new(1.0, (0.0, 0.0));
        // Overlap: the later (top) box wins.
        assert_eq!(pick(&s, v, (60.0, 30.0)), Some(Hit::Item(1)));
        // Only the first box there.
        assert_eq!(pick(&s, v, (10.0, 10.0)), Some(Hit::Item(0)));
        // Empty canvas.
        assert_eq!(pick(&s, v, (500.0, 500.0)), None);
        // Box edges count (inclusive containment).
        assert_eq!(pick(&s, v, (0.0, 0.0)), Some(Hit::Item(0)));
    }

    #[test]
    fn pick_ignores_boundaries_and_zooms_with_the_viewport() {
        let i = item(200.0, 0.0);
        let e = edge(vec![(0.0, 0.0), (100.0, 0.0)]);
        let s = scene(
            vec![i],
            vec![e],
            vec![boundary(-100.0, -100.0, 1000.0, 500.0)],
        );
        let v = Viewport::new(1.0, (0.0, 0.0));
        // Inside the boundary rect but outside items/edges: background.
        assert_eq!(pick(&s, v, (50.0, 60.0)), None);
        // On the edge: hit — and the item elsewhere still wins over it.
        assert_eq!(pick(&s, v, (50.0, 3.0)), Some(Hit::Edge(0)));
        // Just outside the 6 px screen tolerance.
        assert_eq!(pick(&s, v, (50.0, 7.0)), None);
        // The tolerance is SCREEN space: zooming in halves the world
        // distance the same pixel offset means.
        let zoomed = Viewport::new(2.0, (0.0, 0.0));
        assert_eq!(pick(&s, zoomed, (50.0, 5.0)), Some(Hit::Edge(0))); // world 2.5 ≤ 3
        assert_eq!(pick(&s, zoomed, (50.0, 13.0)), None); // world 6.5 > 3
    }

    #[test]
    fn item_over_edge_wins_the_pick() {
        let i = item(0.0, 0.0);
        let e = edge(vec![(-50.0, 25.0), (150.0, 25.0)]);
        let s = scene(vec![i], vec![e], Vec::new());
        let v = Viewport::new(1.0, (0.0, 0.0));
        // The line crosses the box: the box click is the item.
        assert_eq!(pick(&s, v, (50.0, 25.0)), Some(Hit::Item(0)));
    }

    #[test]
    fn items_in_rect_lassoes_by_intersection() {
        fn lasso(x: f64, y: f64, width: f64, height: f64) -> Rect {
            Rect {
                x,
                y,
                width,
                height,
            }
        }
        let mut a = item(0.0, 0.0); // 0..100 × 0..50
        a.id = Id::new("a");
        let mut b = item(220.0, 60.0); // 220..320 × 60..110
        b.id = Id::new("b");
        let mut c = item(450.0, 0.0); // 450..550 × 0..50
        c.id = Id::new("c");
        let s = scene(vec![a, b, c], Vec::new(), Vec::new());
        // Full cover: content order; b clipped at its corner still counts.
        assert_eq!(
            items_in_rect(&s, lasso(-10.0, -10.0, 570.0, 130.0)),
            vec![0, 1, 2]
        );
        // A lasso ending exactly at b's left edge grazes it — strict
        // inequality means touching does NOT select.
        assert_eq!(items_in_rect(&s, lasso(-5.0, -5.0, 225.0, 65.0)), vec![0]);
        assert_eq!(
            items_in_rect(&s, lasso(-5.0, -5.0, 226.0, 66.0)),
            vec![0, 1]
        );
        // A lasso over empty canvas grabs nothing.
        assert_eq!(
            items_in_rect(&s, lasso(120.0, 120.0, 60.0, 20.0)),
            Vec::new()
        );
    }

    #[test]
    fn nearest_waypoint_hits_interior_vertices_only() {
        let line = [(0.0, 0.0), (100.0, 20.0), (200.0, 0.0)];
        // Interior vertex within tolerance → its polyline index.
        assert_eq!(nearest_waypoint((102.0, 21.0), &line, 6.0), Some(1));
        // Endpoints belong to the boxes: never a waypoint hit.
        assert_eq!(nearest_waypoint((1.0, 1.0), &line, 6.0), None);
        // Out of tolerance → None; a straight (no interior) line never hits.
        assert_eq!(nearest_waypoint((100.0, 30.0), &line, 6.0), None);
        assert_eq!(
            nearest_waypoint((100.0, 0.0), &[(0.0, 0.0), (200.0, 0.0)], 6.0),
            None
        );
        // The NEARER of two interior vertices wins.
        let bent = [(0.0, 0.0), (100.0, 0.0), (200.0, 0.0), (300.0, 100.0)];
        assert_eq!(nearest_waypoint((101.0, 1.0), &bent, 6.0), Some(1));
    }

    #[test]
    fn insert_waypoint_splices_the_nearest_segment() {
        // A straight line: the new vertex lands between the endpoints.
        assert_eq!(insert_waypoint((40.0, 3.0), &[(0.0, 0.0), (100.0, 0.0)]), 1);
        // The segment closest to the click gets split.
        let l = [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)];
        assert_eq!(insert_waypoint((95.0, 60.0), &l), 2);
        // Degenerate inputs: nothing to split, the append index comes back.
        assert_eq!(insert_waypoint((0.0, 0.0), &[]), 0);
        assert_eq!(insert_waypoint((5.0, 5.0), &[(1.0, 1.0)]), 1);
    }

    #[test]
    fn edge_pick_is_first_in_tolerance_by_content_order() {
        let first = edge(vec![(0.0, 0.0), (100.0, 0.0)]);
        let second = edge(vec![(0.0, 10.0), (100.0, 10.0)]);
        let s = scene(Vec::new(), vec![first, second], Vec::new());
        let v = Viewport::new(1.0, (0.0, 0.0));
        // Only the first line within tolerance.
        assert_eq!(pick(&s, v, (50.0, 3.0)), Some(Hit::Edge(0)));
        // Only the second line within tolerance.
        assert_eq!(pick(&s, v, (50.0, 8.0)), Some(Hit::Edge(1)));
        // Both within tolerance (4 ≤ 6 and 6 ≤ 6): the earlier content
        // entry wins — deterministic, not distance-ranked.
        assert_eq!(pick(&s, v, (50.0, 4.0)), Some(Hit::Edge(0)));
        // Truly neither: left of both segments' ends, 20+ away from each.
        assert_eq!(pick(&s, v, (-20.0, 5.0)), None);
    }

    #[test]
    fn polyline_distance_handles_degenerate_input() {
        // Empty: unreachable.
        assert!(distance_to_polyline((1.0, 2.0), &[]).is_infinite());
        // Single vertex: point distance.
        assert_eq!(distance_to_polyline((2.0, 3.0), &[(2.0, 0.0)]), 3.0);
        // Zero-length segment: point distance, no divide-by-zero.
        assert_eq!(distance_to_segment((3.0, 4.0), (0.0, 0.0), (0.0, 0.0)), 5.0);
        // Duplicate vertices inside a polyline stay routable.
        assert_eq!(
            distance_to_polyline((5.0, 3.0), &[(0.0, 0.0), (0.0, 0.0), (10.0, 0.0)]),
            3.0
        );
    }

    #[test]
    fn segment_distance_clamps_to_the_segment() {
        // Perpendicular foot inside the segment.
        assert_eq!(
            distance_to_segment((5.0, 3.0), (0.0, 0.0), (10.0, 0.0)),
            3.0
        );
        // Foot beyond the end: endpoint distance (3-4-5 triangle).
        assert_eq!(
            distance_to_segment((13.0, 4.0), (0.0, 0.0), (10.0, 0.0)),
            5.0
        );
        // Foot before the start.
        assert_eq!(
            distance_to_segment((-3.0, -4.0), (0.0, 0.0), (10.0, 0.0)),
            5.0
        );
    }
}
