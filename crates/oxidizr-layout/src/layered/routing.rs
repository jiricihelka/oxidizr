//! The routing half of the layered engine: the port-spread fan, the
//! per-edge route assembly, the channel-lane spread and the side/endpoint
//! geometry. The build/ordering pipeline lives in `skeleton`; the engine
//! entry and the shared constants stay in the parent module.

use std::collections::BTreeMap;

use crate::engine::Point;
use crate::graph::{Anchor, Edge};

use super::{Chain, Vertex};

/// Which box side an endpoint uses, relative to the RANK axis:
/// `Rank` sides are the ones the rank axis runs through (Left/Right
/// horizontally), `Cross` the other pair, `Neutral` an interior chain
/// point with no side at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SideKind {
    Rank,
    Cross,
    Neutral,
}

/// Port spread: every edge endpoint attaches to one of its box's
/// four sides ([`pick_side`] — the exact rule [`endpoint`] applies);
/// edges sharing a (node, side) then SLIDE apart — ordered by the far
/// box centre's coordinate PARALLEL to that side (ties → edge input
/// order, determinism contract). `edge_separation` is the DESIRED
/// distance between neighbouring ports: it fits the group, they sit
/// CENTRED on the side exactly `edge_separation` apart; when it does
/// not (many edges, small box), the side is the hard limit and they
/// fall back to N+1 equally spaced slots along it. The order preserves
/// the far ends' cross order, so the fanned bundle keeps its (already
/// minimised) crossing structure; N = 1 stays on the midpoint, so lone
/// edges route exactly as before.
/// Returns the per-edge `(from, to)` fractions along the attach side.
#[allow(clippy::cast_precision_loss)] // slot ranks/sizes are tiny counts
#[allow(clippy::cast_possible_truncation)] // fan sizes are tiny counts
#[allow(clippy::too_many_arguments)] // the geometry bundle the fan reads
pub(super) fn port_spread(
    chains: &[Chain],
    edges: &[Edge],
    verts: &[Vertex],
    centers: &[Point],
    boxes: &[(f64, f64, f64, f64)],
    generate_vertices: bool,
    edge_separation: f64,
) -> Vec<(f64, f64)> {
    let mut alongs = vec![(0.5, 0.5); chains.len()];
    let mut from_groups: BTreeMap<(usize, Side4), Vec<(f64, usize)>> = BTreeMap::new();
    let mut to_groups: BTreeMap<(usize, Side4), Vec<(f64, usize)>> = BTreeMap::new();
    for (idx, chain) in chains.iter().enumerate() {
        let last = chain.verts.len() - 1;
        let (start_vid, end_vid) = if chain.reversed {
            (chain.verts[last], chain.verts[0])
        } else {
            (chain.verts[0], chain.verts[last])
        };
        let (si, ti) = (verts[start_vid].real, verts[end_vid].real);
        if si == ti {
            continue; // the self-loop stub pair owns its own anchors
        }
        // `toward`/`back` EXACTLY as `route` computes them (the first/
        // last interior point IN THE INPUT DIRECTION, else the far
        // centre) — the side must be the one `endpoint` will pick.
        let skipped: &[usize] = if generate_vertices {
            &chain.verts[1..last]
        } else {
            &[]
        };
        let (first_in_dir, last_in_dir) = if chain.reversed {
            (skipped.last().copied(), skipped.first().copied())
        } else {
            (skipped.first().copied(), skipped.last().copied())
        };
        let toward = first_in_dir.map_or_else(|| center(boxes[ti]), |vid| centers[vid]);
        let back = last_in_dir.map_or_else(|| center(boxes[si]), |vid| centers[vid]);
        let from_side = pick_side(boxes[si], edges[idx].from, toward);
        let to_side = pick_side(boxes[ti], edges[idx].to, back);
        let far_from = center(boxes[ti]);
        let far_to = center(boxes[si]);
        let key = |side: Side4, far: Point| {
            if side.slides_x() { far.x } else { far.y }
        };
        from_groups
            .entry((si, from_side))
            .or_default()
            .push((key(from_side, far_from), idx));
        to_groups
            .entry((ti, to_side))
            .or_default()
            .push((key(to_side, far_to), idx));
    }
    for (groups, which) in [(&to_groups, 1_usize), (&from_groups, 0)] {
        for (&(node_idx, side), members) in groups {
            let n = members.len();
            if n < 2 {
                continue;
            }
            let mut sorted = members.clone();
            sorted.sort_by(|a, b| {
                a.0.partial_cmp(&b.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.1.cmp(&b.1))
            });
            // The side the ports slide along: `edge_separation` is a
            // PIXEL distance, so it competes with the side's length.
            let (_, _, bw, bh) = boxes[node_idx];
            let side_len = if side.slides_x() { bw } else { bh };
            let span = f64::from(n as u32 - 1) * edge_separation;
            // Exact spacing when the requested span FITS the side
            // (centred: the fan keeps the midpoint symmetric); a zero
            // or negative request, or a span that would hang the outer
            // ports off the corners, falls back to the N+1 equal slots.
            let exact = edge_separation > 0.0 && side_len > 0.0 && span <= side_len;
            for (rank, (_, edge_idx)) in sorted.iter().enumerate() {
                let fraction = if exact {
                    let offset = f64::from(rank as u32) * edge_separation - span / 2.0;
                    (0.5 + offset / side_len).clamp(0.0, 1.0)
                } else {
                    (rank + 1) as f64 / (n + 1) as f64
                };
                if which == 0 {
                    alongs[*edge_idx].0 = fraction;
                } else {
                    alongs[*edge_idx].1 = fraction;
                }
            }
        }
    }
    alongs
}

/// One interior (channel) leg of an orthogonal route — a segment whose
/// two neighbours are both perpendicular to it, so both neighbours
/// STRETCH when the leg moves.
#[derive(Clone, Copy)]
struct ChannelLeg {
    /// Fixed coordinate: `x` for a vertical leg, `y` for a horizontal one.
    fixed: f64,
    /// Covered range along its own axis.
    start: f64,
    end: f64,
    /// Lane slack: half the distance to the nearer outer point — the
    /// lane spread never exceeds the SMALLEST such slack in a cluster,
    /// so a lane stays strictly between the leg's two neighbours' far
    /// ends: no degenerate zero-length neighbour, never across a box
    /// side, length-neutral (the outer span is unchanged).
    clamp: f64,
    route: usize,
    seg: usize,
    /// `true` for a vertical leg (fixed `x`, lanes shift `x`).
    upright: bool,
}

/// Channel-lane pass: [`push_segment`] bends EVERY diagonal
/// through the exact midpoint of the two legs it connects, so parallel
/// legs between repeating box sizes land on one coordinate and overlap.
/// This post-pass groups all channel legs by (orientation, exact fixed
/// coordinate), clusters collinearly OVERLAPPING legs, and spreads each
/// cluster of `k` into distinct lanes around the shared coordinate. The
/// total spread fits both the gap the legs sit in (`rank_separation`
/// for legs crossing rank gaps, `node_separation` for cross-side ones;
/// a zero gap is a deterministic no-op) and twice the smallest per-leg
/// slack (half the distance to the nearer outer point), so every lane
/// fits every leg: the move never degenerates a neighbour to zero
/// length, never crosses a box side, and is length-neutral. Lanes
/// preserve orthogonality by construction
/// (both leg endpoints shift on the same axis). Deterministic: `to_bits`
/// group keys, `total_cmp` ordering, fixed member order (range start,
/// range end, route, segment). Residual touches against a static leg at
/// a lane coordinate stay — the pass resolves bundles ON a shared bend,
/// the dominant real-world overlap.
#[allow(clippy::cast_precision_loss)] // cluster sizes are tiny counts
#[allow(clippy::float_cmp)] // shared-coordinate runs are exact-value tests
// by design (determinism) — same rationale as `push_segment`.
pub(super) fn spread_channels(
    routes: &mut [Vec<Point>],
    horizontal: bool,
    rank_sep: f64,
    node_sep: f64,
) {
    let mut legs: Vec<ChannelLeg> = Vec::new();
    for (ri, pts) in routes.iter().enumerate() {
        for si in 1..pts.len().saturating_sub(2) {
            let (p, q) = (pts[si], pts[si + 1]);
            let (prev, next) = (pts[si - 1], pts[si + 2]);
            let vertical_leg = p.x == q.x;
            let perpendicular = if vertical_leg {
                prev.y == p.y && next.y == q.y
            } else {
                prev.x == p.x && next.x == q.x
            };
            if !perpendicular {
                continue; // not a bend leg — a spine stub or a straight run
            }
            legs.push(if vertical_leg {
                ChannelLeg {
                    fixed: p.x,
                    start: p.y.min(q.y),
                    end: p.y.max(q.y),
                    clamp: (p.x - prev.x).abs().min((next.x - q.x).abs()) / 2.0,
                    route: ri,
                    seg: si,
                    upright: true,
                }
            } else {
                ChannelLeg {
                    fixed: p.y,
                    start: p.x.min(q.x),
                    end: p.x.max(q.x),
                    clamp: (p.y - prev.y).abs().min((next.y - q.y).abs()) / 2.0,
                    route: ri,
                    seg: si,
                    upright: false,
                }
            });
        }
    }
    // One sort does the grouping AND the intra-group cluster ordering:
    // equal (orientation, fixed coordinate) legs end up adjacent, ordered
    // by range start — the scan below only walks runs.
    legs.sort_by(|a, b| {
        a.upright
            .cmp(&b.upright)
            .then(a.fixed.to_bits().cmp(&b.fixed.to_bits()))
            .then(a.start.total_cmp(&b.start))
            .then(a.end.total_cmp(&b.end))
            .then(a.route.cmp(&b.route))
            .then(a.seg.cmp(&b.seg))
    });
    let mut i = 0;
    while i < legs.len() {
        let key = (legs[i].upright, legs[i].fixed.to_bits());
        let mut g = i + 1;
        while g < legs.len() && (legs[g].upright, legs[g].fixed.to_bits()) == key {
            g += 1;
        }
        let cap = if key.0 == horizontal {
            rank_sep // vertical legs across horizontal rank gaps (and mirror)
        } else {
            node_sep
        };
        if cap > 0.0 {
            // Overlap clusters inside the shared-coordinate group.
            let mut k = i;
            while k < g {
                let mut m = k + 1;
                let mut run_end = legs[k].end;
                while m < g && legs[m].start < run_end {
                    run_end = run_end.max(legs[m].end);
                    m += 1;
                }
                if m - k > 1 {
                    // Total spread ≤ the gap, and ≤ TWICE the smallest
                    // leg slack in the cluster — so every lane fits in
                    // EVERY leg (no clamp pile-ups, all lanes distinct).
                    let slack = legs[k..m]
                        .iter()
                        .map(|leg| leg.clamp)
                        .fold(f64::INFINITY, f64::min)
                        .min(cap / 2.0);
                    let lane = 2.0 * slack / (m - k - 1) as f64;
                    for (o, leg) in legs[k..m].iter().enumerate() {
                        let off = (o as f64 - (m - k - 1) as f64 / 2.0) * lane;
                        let pts = &mut routes[leg.route];
                        if leg.upright {
                            pts[leg.seg].x += off;
                            pts[leg.seg + 1].x += off;
                        } else {
                            pts[leg.seg].y += off;
                            pts[leg.seg + 1].y += off;
                        }
                    }
                }
                k = m;
            }
        }
        i = g;
    }
}

/// Step 6: anchor → dummy centres (only when `generate_vertices`) →
/// anchor, walked in the INPUT direction — chains are stored
/// acyclic-oriented, so a reversed edge's chain runs backwards here.
/// Diagonal legs are orthogonalised through channels at the mid rank
/// coordinate — a rank GAP, never a rank band — or at the mid cross
/// coordinate when both ends attach to the cross sides (the
/// "orthogonal-ish" MVP; polish is a later pass). Without
/// `generate_vertices` the route is the straight anchor→anchor pair.
#[allow(clippy::too_many_arguments)] // one structural bundle per argument
pub(super) fn route(
    edge: &Edge,
    chain: &Chain,
    verts: &[Vertex],
    centers: &[Point],
    boxes: &[(f64, f64, f64, f64)],
    generate_vertices: bool,
    horizontal: bool,
    along: (f64, f64),
) -> Vec<Point> {
    let last = chain.verts.len() - 1;
    let (start_vid, end_vid) = if chain.reversed {
        (chain.verts[last], chain.verts[0])
    } else {
        (chain.verts[0], chain.verts[last])
    };
    let (si, ti) = (verts[start_vid].real, verts[end_vid].real);
    if si == ti {
        // Self-loop placeholder route: two stubs on
        // the box's right side — deterministic and bounded.
        let (bx, by, bw, bh) = boxes[si];
        return vec![
            Point {
                x: bx + bw,
                y: by + bh * 0.25,
            },
            Point {
                x: bx + bw,
                y: by + bh * 0.75,
            },
        ];
    }
    let interior: Vec<usize> = if generate_vertices {
        let skipped = &chain.verts[1..last];
        if chain.reversed {
            skipped.iter().rev().copied().collect()
        } else {
            skipped.to_vec()
        }
    } else {
        Vec::new()
    };
    let toward = interior
        .first()
        .map_or_else(|| center(boxes[ti]), |&vid| centers[vid]);
    let from = endpoint(boxes[si], edge.from, toward, horizontal, along.0);
    let back = interior
        .last()
        .map_or_else(|| center(boxes[si]), |&vid| centers[vid]);
    let to = endpoint(boxes[ti], edge.to, back, horizontal, along.1);
    if !generate_vertices {
        return vec![from.0, to.0];
    }
    let mut spine: Vec<(Point, SideKind)> = Vec::with_capacity(interior.len() + 2);
    spine.push(from);
    spine.extend(
        interior
            .iter()
            .map(|&vid| (centers[vid], SideKind::Neutral)),
    );
    spine.push(to);
    let mut points: Vec<Point> = Vec::with_capacity(spine.len() * 3);
    points.push(spine[0].0);
    for pair in spine.windows(2) {
        push_segment(&mut points, pair[0], pair[1], horizontal);
    }
    collapse(&mut points);
    points
}

/// One route leg, orthogonalised: axis-parallel legs stay direct;
/// diagonals bend through the mid RANK coordinate (a rank-gap
/// channel), or through the mid CROSS coordinate when both ends
/// attach to the cross sides, so the leg leaves and enters
/// perpendicular to those sides.
#[allow(clippy::float_cmp)]
// Comparing exact layout values (integer separations and exact
// half-sums, determinism): equality here is the defined test
// for an axis-parallel leg, not a fuzzy comparison.
fn push_segment(
    points: &mut Vec<Point>,
    from: (Point, SideKind),
    to: (Point, SideKind),
    horizontal: bool,
) {
    let (p, q) = (from.0, to.0);
    if p.x == q.x || p.y == q.y {
        points.push(q);
        return;
    }
    let cross_both = from.1 == SideKind::Cross && to.1 == SideKind::Cross;
    if cross_both == horizontal {
        // Channel at the mid CROSS coordinate.
        if horizontal {
            let mid = p.y.midpoint(q.y);
            points.push(Point { x: p.x, y: mid });
            points.push(Point { x: q.x, y: mid });
        } else {
            let mid = p.x.midpoint(q.x);
            points.push(Point { x: mid, y: p.y });
            points.push(Point { x: mid, y: q.y });
        }
    } else if horizontal {
        let mid = p.x.midpoint(q.x);
        points.push(Point { x: mid, y: p.y });
        points.push(Point { x: mid, y: q.y });
    } else {
        let mid = p.y.midpoint(q.y);
        points.push(Point { x: p.x, y: mid });
        points.push(Point { x: q.x, y: mid });
    }
    points.push(q);
}

/// Drop zero-length steps and the middle point of straight runs
/// (three consecutive points sharing an x or a y) — bend channels
/// that coincide with an endpoint, or dummy chains that the priority
/// passes aligned into one line, collapse back to the shortest
/// polyline stating the same path.
#[allow(clippy::float_cmp)]
// Ditto: shared-coordinate runs are exact-value tests by design.
fn collapse(points: &mut Vec<Point>) {
    let mut kept: Vec<Point> = Vec::with_capacity(points.len());
    for point in points.iter().copied() {
        if kept.last() == Some(&point) {
            continue;
        }
        kept.push(point);
        let len = kept.len();
        if len >= 3 {
            let (first, mid, last) = (kept[len - 3], kept[len - 2], kept[len - 1]);
            if (first.x == mid.x && mid.x == last.x) || (first.y == mid.y && mid.y == last.y) {
                kept.remove(len - 2);
            }
        }
    }
    *points = kept;
}

/// Centre of a world-space box.
fn center(box_rect: (f64, f64, f64, f64)) -> Point {
    let (x, y, w, h) = box_rect;
    Point {
        x: x + w / 2.0,
        y: y + h / 2.0,
    }
}

/// The four box sides, as an anchor attaches to one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Side4 {
    Top,
    Bottom,
    Left,
    Right,
}

impl Side4 {
    /// Whether the anchor SLIDES along this side by the x coordinate
    /// (Top/Bottom) or the y coordinate (Left/Right).
    fn slides_x(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }
}

/// The side of `box_rect` an anchor attaches to: the named side when
/// pinned, otherwise the side facing `toward` (dominant delta wins,
/// exact ties take the horizontal sides). Shared by [`endpoint`] and
/// the port-spread prepass so BOTH agree which side an edge
/// lands on (the spread order is only meaningful if it matches).
fn pick_side(box_rect: (f64, f64, f64, f64), anchor: Option<Anchor>, toward: Point) -> Side4 {
    match anchor {
        Some(Anchor::Top) => Side4::Top,
        Some(Anchor::Bottom) => Side4::Bottom,
        Some(Anchor::Left) => Side4::Left,
        Some(Anchor::Right) => Side4::Right,
        None => {
            let mid = center(box_rect);
            let dx = toward.x - mid.x;
            let dy = toward.y - mid.y;
            if dx.abs() >= dy.abs() {
                if dx >= 0.0 { Side4::Right } else { Side4::Left }
            } else if dy >= 0.0 {
                Side4::Bottom
            } else {
                Side4::Top
            }
        }
    }
}

/// The anchor point of `box_rect` with its side kind: the side from
/// [`pick_side`], positioned `along` it (0 = one end, 1 = the other,
/// ½ = the midpoint the pinned anchors keep). The port spread feeds a
/// per-edge `along` so edges sharing a (node, side) fan out instead of
/// stacking on the midpoint; a lone edge keeps ½ (the midpoint).
fn endpoint(
    box_rect: (f64, f64, f64, f64),
    anchor: Option<Anchor>,
    toward: Point,
    horizontal: bool,
    along: f64,
) -> (Point, SideKind) {
    let (bx, by, bw, bh) = box_rect;
    let (rank_side, cross_side) = if horizontal {
        (SideKind::Rank, SideKind::Cross)
    } else {
        (SideKind::Cross, SideKind::Rank)
    };
    let point = |side: Side4| match side {
        Side4::Top => Point {
            x: bx + along * bw,
            y: by,
        },
        Side4::Bottom => Point {
            x: bx + along * bw,
            y: by + bh,
        },
        Side4::Left => Point {
            x: bx,
            y: by + along * bh,
        },
        Side4::Right => Point {
            x: bx + bw,
            y: by + along * bh,
        },
    };
    let side = pick_side(box_rect, anchor, toward);
    // The leg axis keys off the SIDE (horizontal sides ride the rank
    // axis, vertical sides the cross axis): the dominant-delta side
    // chooses rank vs cross.
    let kind = if side.slides_x() {
        cross_side
    } else {
        rank_side
    };
    (point(side), kind)
}
