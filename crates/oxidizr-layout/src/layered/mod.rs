//! [`Layered`] — the Sugiyama-family layered engine (**MVP scope**: the
//! core pipeline; quality polish is a later pass).
//!
//! Pipeline, in fixed order:
//!
//! 1. **cycle removal** — iterative DFS over the graph in node/edge
//!    insertion order; every back edge is reversed and remembered
//!    (self-loops are kept in place and route specially);
//! 2. **layering** — longest-path ranks over the acyclic orientation
//!    (Kahn in insertion order; the "did it converge" check is
//!    defensive — DFS reversal already guarantees a DAG; there is no
//!    simplex refinement at this scope);
//! 3. **long edges** — one zero-size dummy vertex per rank the edge
//!    skips, chained into the edge's route;
//! 4. **ordering** — barycenter sweeps: fixed rounds, each sweeping
//!    the ranks down (barycentre from the rank above) and up (from
//!    the rank below), ties falling to the current position through
//!    the stable sort; the order with the fewest crossings wins
//!    (ties keep the earlier order, so the initial one);
//! 5. **coordinates** — ranks advance by the rank axis' largest node
//!    plus `rank_separation` and stack on the cross axis with
//!    `node_separation`, then the **priority method** refines the
//!    cross positions: fixed alternating passes move each vertex
//!    toward the mean centre of one rank side's neighbours — dummies
//!    first, so long-edge chains straighten — clamped into the slot
//!    its within-rank neighbours leave open (order and separations
//!    preserved); finally the lowest box returns to the zero line;
//! 6. **edge routes** — polyline anchor → dummy centres → anchor,
//!    orthogonalised through rank-gap channels, interior points
//!    emitted only when `generate_vertices`; a final **channel-lane
//!    pass** then pulls parallel bend legs that landed on
//!    one shared midpoint apart into distinct lanes.
//!
//! `rank_direction` decides which world axis is the rank axis and
//! which way it runs: ranks always advance left→right / top→down in
//! the canonical frame, mirrored for `RightLeft` / `BottomUp`.
//!
//! **Determinism (hard contract):** no randomness, stable
//! sorts only, fixed passes — the same input graph always produces an
//! identical [`LayoutResult`].

use crate::engine::{LayoutEngine, LayoutError, LayoutResult};
use crate::graph::LayoutGraph;
mod skeleton;

use skeleton::{Chain, Skeleton, Vertex};
mod routing;

/// Barycenter rounds (pipeline step 4): each round is one down + one
/// up sweep. Fixed count — determinism needs fixed passes.
const BARYCENTER_ROUNDS: usize = 4;

/// Priority-method passes (pipeline step 5), alternating the neighbour
/// side (rank-1, then rank+1). Fixed count, ditto.
const PRIORITY_ROUNDS: usize = 4;

/// The layered engine. Stateless today; kept as a unit struct
/// so [`LayoutEngine`] stays object-safe for future configuration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Layered;

impl Layered {
    /// The engine instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl LayoutEngine for Layered {
    fn layout(&self, graph: &LayoutGraph) -> Result<LayoutResult, LayoutError> {
        graph.validate()?;
        if graph.nodes.is_empty() {
            return Ok(LayoutResult::default());
        }
        let skel = Skeleton::build(graph)?;
        Ok(skel.place(graph))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::engine::Point;
    use crate::graph::{Anchor, Edge, EdgeId, Node, NodeId, RankDirection};
    use crate::layered::routing::spread_channels;
    use std::collections::BTreeMap;

    fn node(id: &str) -> Node {
        Node::new(NodeId::new(id), 120.0, 60.0)
    }

    fn edge(id: &str, from: &str, to: &str) -> Edge {
        Edge::new(EdgeId::new(id), NodeId::new(from), NodeId::new(to))
    }

    fn layout(graph: &LayoutGraph) -> LayoutResult {
        Layered.layout(graph).expect("well-formed graph lays out")
    }

    fn pos(r: &LayoutResult, id: &str) -> Point {
        r.nodes
            .iter()
            .find(|p| p.id.as_str() == id)
            .expect("node placed")
            .position
    }

    fn pts(r: &LayoutResult, id: &str) -> Vec<Point> {
        r.edges
            .iter()
            .find(|p| p.id.as_str() == id)
            .expect("edge routed")
            .points
            .clone()
    }

    #[test]
    fn empty_graph_places_nothing() {
        assert_eq!(layout(&LayoutGraph::new()), LayoutResult::default());
    }

    #[test]
    fn single_node_at_origin() {
        let mut g = LayoutGraph::new();
        g.add_node(node("a"));
        assert_eq!(pos(&layout(&g), "a"), Point { x: 0.0, y: 0.0 });
    }

    #[test]
    fn chain_lays_out_left_to_right_at_reference_separations() {
        let mut g = LayoutGraph::new();
        for id in ["a", "b", "c"] {
            g.add_node(node(id));
        }
        g.add_edge(edge("ab", "a", "b"));
        g.add_edge(edge("bc", "b", "c"));
        let r = layout(&g);
        assert_eq!(pos(&r, "a"), Point { x: 0.0, y: 0.0 });
        assert_eq!(pos(&r, "b"), Point { x: 220.0, y: 0.0 });
        assert_eq!(pos(&r, "c"), Point { x: 440.0, y: 0.0 });
        // rank_separation (100) between the rank boxes…
        assert_eq!(pos(&r, "b").x - (pos(&r, "a").x + 120.0), 100.0);
        // …and anchor points on the facing box sides.
        assert_eq!(
            pts(&r, "ab"),
            vec![Point { x: 120.0, y: 30.0 }, Point { x: 220.0, y: 30.0 }]
        );
    }

    #[test]
    fn diamond_ranks_and_centres() {
        let mut g = LayoutGraph::new();
        for id in ["a", "b", "c", "d"] {
            g.add_node(node(id));
        }
        for (id, s, t) in [
            ("ab", "a", "b"),
            ("ac", "a", "c"),
            ("bd", "b", "d"),
            ("cd", "c", "d"),
        ] {
            g.add_edge(edge(id, s, t));
        }
        let r = layout(&g);
        assert_eq!(pos(&r, "a"), Point { x: 0.0, y: 55.0 });
        assert_eq!(pos(&r, "b"), Point { x: 220.0, y: 0.0 });
        assert_eq!(pos(&r, "c"), Point { x: 220.0, y: 110.0 });
        assert_eq!(pos(&r, "d"), Point { x: 440.0, y: 55.0 });
        // node_separation (50) between the same-rank boxes.
        assert_eq!(pos(&r, "c").y - (pos(&r, "b").y + 60.0), 50.0);
    }

    #[test]
    fn cycle_is_broken_into_forward_ranks() {
        let mut g = LayoutGraph::new();
        for id in ["a", "b", "c"] {
            g.add_node(node(id));
        }
        g.add_edge(edge("ab", "a", "b"));
        g.add_edge(edge("bc", "b", "c"));
        g.add_edge(edge("ca", "c", "a")); // back edge -> reversed
        let r = layout(&g);
        // a DAG order a < b < c along the rank axis.
        assert!(pos(&r, "a").x < pos(&r, "b").x);
        assert!(pos(&r, "b").x < pos(&r, "c").x);
        // The reversed edge still ROUTES input direction c → a — and,
        // being acyclically a → c across rank 1, it passes through the
        // dummy's channel at y 110 (the priority passes centre a and c
        // at 70: each is pulled toward mean(b=30, dummy=110) by the
        // rank-side neighbours, and clamped by rank-order slots). The
        // legs are orthogonal: every pair shares x or y.
        assert_eq!(
            pts(&r, "ca"),
            vec![
                Point { x: 440.0, y: 70.0 },
                Point { x: 360.0, y: 70.0 },
                Point { x: 360.0, y: 110.0 },
                Point { x: 200.0, y: 110.0 },
                Point { x: 200.0, y: 70.0 },
                Point { x: 120.0, y: 70.0 },
            ]
        );
    }

    #[test]
    fn self_loop_routes_two_stubs() {
        let mut g = LayoutGraph::new();
        g.add_node(node("a"));
        g.add_edge(edge("aa", "a", "a"));
        let stubs = pts(&layout(&g), "aa");
        assert_eq!(
            stubs,
            vec![Point { x: 120.0, y: 15.0 }, Point { x: 120.0, y: 45.0 }]
        );
    }

    #[test]
    fn long_edge_gets_a_dummy_waypoint() {
        let mut g = LayoutGraph::new();
        for id in ["a", "b", "c"] {
            g.add_node(node(id));
        }
        g.add_edge(edge("ab", "a", "b"));
        g.add_edge(edge("bc", "b", "c"));
        g.add_edge(edge("ac", "a", "c")); // spans rank 1: one dummy
        let r = layout(&g);
        // The priority method centres a/c at 70 (mean of b=30 and the
        // dummy, clamped by the rank-1 slot: b tops it, the zero-size
        // dummy sits at its 110 centre) — exact floats all round.
        assert_eq!(pos(&r, "a"), Point { x: 0.0, y: 40.0 });
        assert_eq!(pos(&r, "b"), Point { x: 220.0, y: 0.0 });
        assert_eq!(pos(&r, "c"), Point { x: 440.0, y: 40.0 });
        // Orthogonal route a → dummy → c: two rank-gap channels at
        // x 200 / x 360 around the dummy's 110 line. Port
        // spread: a's RIGHT side hosts a→b too and c's LEFT side hosts
        // b→c, so a→c takes the lower slot of both sides (its far ends
        // sit below b's centre line) — y 80 instead of the 70 midpoint.
        assert_eq!(
            pts(&r, "ac"),
            vec![
                Point { x: 120.0, y: 80.0 },
                Point { x: 200.0, y: 80.0 },
                Point { x: 200.0, y: 110.0 },
                Point { x: 360.0, y: 110.0 },
                Point { x: 360.0, y: 80.0 },
                Point { x: 440.0, y: 80.0 },
            ]
        );
    }

    #[test]
    fn edges_sharing_a_side_spread_into_thirds() {
        // Port spread (arrows "should prefer to not land in
        // each other"): a→h and b→h both enter h's LEFT side, so they
        // take the ⅓ / ⅔ slots in source-centre order — never the
        // shared midpoint.
        let mut g = LayoutGraph::new();
        for id in ["a", "b", "h"] {
            g.add_node(node(id));
        }
        g.add_edge(edge("ah", "a", "h"));
        g.add_edge(edge("bh", "b", "h"));
        let r = layout(&g);
        let h = pos(&r, "h");
        let mut ends = [
            *pts(&r, "ah").last().unwrap(),
            *pts(&r, "bh").last().unwrap(),
        ];
        ends.sort_by(|p, q| p.y.total_cmp(&q.y));
        assert_eq!(
            ends[0],
            Point {
                x: h.x,
                y: h.y + 20.0
            }
        );
        assert_eq!(
            ends[1],
            Point {
                x: h.x,
                y: h.y + 40.0
            }
        );
    }

    #[test]
    fn without_vertices_routes_are_straight() {
        let mut g = LayoutGraph::new();
        for id in ["a", "b", "c"] {
            g.add_node(node(id));
        }
        g.add_edge(edge("ab", "a", "b"));
        g.add_edge(edge("bc", "b", "c"));
        g.add_edge(edge("ac", "a", "c"));
        g.config.generate_vertices = false;
        assert_eq!(pts(&layout(&g), "ac").len(), 2);
    }

    #[test]
    fn anchors_pin_the_named_sides() {
        let mut g = LayoutGraph::new();
        g.add_node(node("a"));
        g.add_node(node("b"));
        let mut e = edge("ab", "a", "b");
        e.from = Some(Anchor::Top);
        e.to = Some(Anchor::Bottom);
        g.add_edge(e);
        // Both ends pin a CROSS side (Top/Bottom horizontally), so the
        // diagonal gets the cross-channel Z at mid y (0+60)/2 = 30.
        assert_eq!(
            pts(&layout(&g), "ab"),
            vec![
                Point { x: 60.0, y: 0.0 },
                Point { x: 60.0, y: 30.0 },
                Point { x: 280.0, y: 30.0 },
                Point { x: 280.0, y: 60.0 },
            ]
        );
    }

    #[test]
    fn barycenter_untangles_crossing_layers() {
        // u0→v1 + u1→v0: the initial (node-index) order crosses once;
        // the sweeps must untangle it (a crossing-free order exists).
        let mut g = LayoutGraph::new();
        for id in ["u0", "u1", "v0", "v1"] {
            g.add_node(node(id));
        }
        g.add_edge(edge("e0", "u0", "v1"));
        g.add_edge(edge("e1", "u1", "v0"));
        let r = layout(&g);
        assert_eq!(pos(&r, "u0"), Point { x: 0.0, y: 0.0 });
        assert_eq!(pos(&r, "u1"), Point { x: 0.0, y: 110.0 });
        // v1 above v0 — the crossed order is gone…
        assert_eq!(pos(&r, "v1"), Point { x: 220.0, y: 0.0 });
        assert_eq!(pos(&r, "v0"), Point { x: 220.0, y: 110.0 });
        // …and both routes are straight horizontals.
        assert_eq!(
            pts(&r, "e0"),
            vec![Point { x: 120.0, y: 30.0 }, Point { x: 220.0, y: 30.0 }]
        );
        assert_eq!(
            pts(&r, "e1"),
            vec![Point { x: 120.0, y: 140.0 }, Point { x: 220.0, y: 140.0 }]
        );
    }

    #[test]
    #[allow(clippy::float_cmp)] // exact layout values by design
    fn routes_are_axis_aligned() {
        // The orthogonal-ish promise: with vertices generated,
        // every route leg of a mixed graph shares x or y.
        let mut g = LayoutGraph::new();
        for id in ["a", "b", "c", "d"] {
            g.add_node(node(id));
        }
        for (id, s, t) in [
            ("ab", "a", "b"),
            ("ac", "a", "c"),
            ("bd", "b", "d"),
            ("cd", "c", "d"),
        ] {
            g.add_edge(edge(id, s, t));
        }
        g.add_edge(edge("ad", "a", "d")); // spans two ranks
        for direction in [
            RankDirection::LeftRight,
            RankDirection::RightLeft,
            RankDirection::TopDown,
            RankDirection::BottomUp,
        ] {
            g.config.rank_direction = direction;
            let r = layout(&g);
            for placement in &r.edges {
                for pair in placement.points.windows(2) {
                    assert!(
                        pair[0].x == pair[1].x || pair[0].y == pair[1].y,
                        "{direction:?} edge {} leg {:?}→{:?} is diagonal",
                        placement.id,
                        pair[0],
                        pair[1]
                    );
                }
            }
        }
    }

    #[test]
    fn topdown_runs_the_rank_axis_down() {
        let mut g = LayoutGraph::new();
        g.add_node(node("a"));
        g.add_node(node("b"));
        g.add_edge(edge("ab", "a", "b"));
        g.config.rank_direction = RankDirection::TopDown;
        let r = layout(&g);
        assert_eq!(pos(&r, "a"), Point { x: 0.0, y: 0.0 });
        assert_eq!(pos(&r, "b"), Point { x: 0.0, y: 160.0 });
        assert_eq!(
            pts(&r, "ab"),
            vec![Point { x: 60.0, y: 60.0 }, Point { x: 60.0, y: 160.0 },]
        );
    }

    #[test]
    fn rightleft_mirrors_the_canonical_frame() {
        let mut g = LayoutGraph::new();
        for id in ["a", "b", "c"] {
            g.add_node(node(id));
        }
        g.add_edge(edge("ab", "a", "b"));
        g.add_edge(edge("bc", "b", "c"));
        g.config.rank_direction = RankDirection::RightLeft;
        let r = layout(&g);
        assert!(pos(&r, "a").x > pos(&r, "b").x);
        assert!(pos(&r, "b").x > pos(&r, "c").x);
    }

    #[test]
    fn disconnected_nodes_share_the_first_rank() {
        let mut g = LayoutGraph::new();
        for id in ["a", "b", "c"] {
            g.add_node(node(id));
        }
        let r = layout(&g);
        assert_eq!(pos(&r, "a"), Point { x: 0.0, y: 0.0 });
        assert_eq!(pos(&r, "b"), Point { x: 0.0, y: 110.0 });
        assert_eq!(pos(&r, "c"), Point { x: 0.0, y: 220.0 });
    }

    #[test]
    fn validation_errors_propagate() {
        let mut g = LayoutGraph::new();
        g.add_node(node("a"));
        g.add_edge(edge("x", "a", "ghost"));
        assert!(matches!(
            Layered.layout(&g),
            Err(LayoutError::UnknownEdgeEndpoint { .. })
        ));
    }

    #[test]
    fn same_input_lays_out_identically() {
        // The hard determinism contract: a mid-size cyclic
        // synthetic, laid out twice.
        let mut g = LayoutGraph::new();
        let count = 60_usize;
        for i in 0..count {
            g.add_node(node(&format!("n{i}")));
        }
        for i in 0..count {
            g.add_edge(edge(
                &format!("e{i}"),
                &format!("n{i}"),
                &format!("n{}", (i + 1 + i % 7) % count),
            ));
        }
        assert_eq!(layout(&g), layout(&g));
    }

    #[test]
    fn thousand_nodes_are_deterministic() {
        // Scale gate: the whole pipeline (sweeps, crossing
        // counts, priority passes, orthogonal routes) on 1k nodes —
        // twice, byte-identical, all coordinates finite.
        let mut g = LayoutGraph::new();
        let count = 1000_usize;
        for i in 0..count {
            g.add_node(node(&format!("n{i}")));
        }
        for i in 0..count {
            // Mixed fan-in/out + a back edge every 11th: cycles,
            // long spans and dummies all exercised.
            let target = (i + 1 + i % 7) % count;
            g.add_edge(edge(
                &format!("e{i}"),
                &format!("n{i}"),
                &format!("n{target}"),
            ));
            if i % 11 == 0 {
                let back = i.saturating_sub(23);
                g.add_edge(edge(
                    &format!("b{i}"),
                    &format!("n{i}"),
                    &format!("n{back}"),
                ));
            }
        }
        let once = layout(&g);
        let twice = layout(&g);
        assert_eq!(once, twice);
        assert_eq!(once.nodes.len(), count);
        assert!(
            once.nodes
                .iter()
                .all(|p| p.position.x.is_finite() && p.position.y.is_finite())
        );
        assert!(once.edges.iter().all(|e| {
            e.points
                .iter()
                .all(|pt| pt.x.is_finite() && pt.y.is_finite())
        }));
        // The lane pass leaves no colliding channel legs even at
        // 1k-node scale (both maps: rank and cross channels).
        assert_eq!(
            overlapping_channel_pairs(&once),
            0,
            "dense mixed graph keeps colliding channels"
        );
    }

    /// Counts channel legs (bend segments whose two neighbours are both
    /// perpendicular — exactly what [`spread_channels`] may move) sitting
    /// on one exact coordinate with strictly overlapping ranges.
    #[allow(clippy::float_cmp)] // exact shared-coordinate test (determinism)
    fn overlapping_channel_pairs(result: &LayoutResult) -> usize {
        let mut pairs = 0_usize;
        for vertical in [true, false] {
            let mut groups: BTreeMap<u64, Vec<(f64, f64)>> = BTreeMap::new();
            for route in &result.edges {
                let pts = &route.points;
                for si in 1..pts.len().saturating_sub(2) {
                    let (p, q) = (pts[si], pts[si + 1]);
                    let (prev, next) = (pts[si - 1], pts[si + 2]);
                    let is_vertical = p.x == q.x;
                    let perpendicular = if is_vertical {
                        prev.y == p.y && next.y == q.y
                    } else {
                        prev.x == p.x && next.x == q.x
                    };
                    if is_vertical != vertical || !perpendicular {
                        continue;
                    }
                    let (fixed, start, end) = if is_vertical {
                        (p.x, p.y.min(q.y), p.y.max(q.y))
                    } else {
                        (p.y, p.x.min(q.x), p.x.max(q.x))
                    };
                    groups
                        .entry(fixed.to_bits())
                        .or_default()
                        .push((start, end));
                }
            }
            for ranges in groups.values() {
                let mut sorted = ranges.clone();
                sorted.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
                for (i, a) in sorted.iter().enumerate() {
                    for b in &sorted[i + 1..] {
                        if b.0 >= a.1 {
                            break;
                        }
                        pairs += 1;
                    }
                }
            }
        }
        pairs
    }

    #[test]
    fn ranks_are_tightest_feasible() {
        // Reality check: step 2 already relaxation-layers
        // to `max(preds) + 1`, so a lone edge beside a longer chain sits
        // tight at rank 1 — there is no slack left to pull.
        let mut g = LayoutGraph::new();
        for id in ["a", "b", "c", "d", "e", "f"] {
            g.add_node(node(id));
        }
        for (id, s, t) in [("ab", "a", "b"), ("bc", "b", "c"), ("de", "d", "e")] {
            g.add_edge(edge(id, s, t));
        }
        let r = layout(&g);
        assert_eq!(pos(&r, "b").x, 220.0);
        assert_eq!(pos(&r, "e").x, 220.0, "the lone edge sits at rank 1");
        assert_eq!(pos(&r, "c").x, 440.0);
        assert_eq!(pos(&r, "f").x, 0.0, "the isolated node keeps rank 0");
    }

    #[test]
    fn channel_lanes_split_an_overlapping_bundle() {
        let bend = |y0, y1| {
            vec![
                Point { x: 120.0, y: y0 },
                Point { x: 220.0, y: y0 },
                Point { x: 220.0, y: y1 },
                Point { x: 320.0, y: y1 },
            ]
        };
        let mut routes = vec![bend(30.0, 90.0), bend(50.0, 70.0)];
        spread_channels(&mut routes, true, 100.0, 50.0);
        // One 100-wide rank gap → two lanes at ±50 around the shared bend.
        assert_eq!(routes[0][1], Point { x: 170.0, y: 30.0 });
        assert_eq!(routes[0][2], Point { x: 170.0, y: 90.0 });
        assert_eq!(routes[1][1], Point { x: 270.0, y: 50.0 });
        assert_eq!(routes[1][2], Point { x: 270.0, y: 70.0 });
        // Outer anchors untouched: the move is length-neutral, box-safe.
        assert_eq!(routes[0][0], Point { x: 120.0, y: 30.0 });
        assert_eq!(routes[1][3], Point { x: 320.0, y: 70.0 });
    }

    #[test]
    fn lane_pass_no_ops_on_lone_channels_and_zero_gaps() {
        let bend = |y0, y1| {
            vec![
                Point { x: 120.0, y: y0 },
                Point { x: 220.0, y: y0 },
                Point { x: 220.0, y: y1 },
                Point { x: 320.0, y: y1 },
            ]
        };
        let lone = vec![bend(30.0, 90.0)];
        let mut single = vec![bend(30.0, 90.0)];
        spread_channels(&mut single, true, 100.0, 50.0);
        assert_eq!(single, lone, "one channel stays on its midpoint");
        let mut starved = vec![bend(30.0, 90.0), bend(50.0, 70.0)];
        spread_channels(&mut starved, true, 0.0, 50.0);
        assert_eq!(starved[0], lone[0], "zero rank gap leaves no lane room");
    }

    #[test]
    fn star_concentration_gets_distinct_lanes() {
        // A star shape in miniature: 59 edges share the hub's
        // one attach side and the same bend midpoint — every vertical
        // channel leg collides until the lane pass spreads them.
        let mut g = LayoutGraph::new();
        for i in 0..60_u32 {
            g.add_node(node(&format!("n{i}")));
        }
        for i in 1..60_u32 {
            g.add_edge(edge(&format!("e{i}"), "n0", &format!("n{i}")));
        }
        let r = layout(&g);
        assert_eq!(overlapping_channel_pairs(&r), 0, "star keeps collisions");
        // The lanes moved: range-colliding legs LEFT the shared midpoint
        // (170 = mid of the hub's right side at 120 and the targets' left
        // edge at 220). Range-disjoint legs may keep it — no overlap.
        let mids: Vec<u64> = r
            .edges
            .iter()
            .filter(|e| e.points.len() == 4)
            .map(|e| e.points[1].x.to_bits())
            .collect();
        assert!(mids.len() > 40, "the fan routes most edges as Z legs");
        assert!(
            mids.iter().any(|b| *b != (170.0_f64).to_bits()),
            "the colliding fan never left the shared midpoint"
        );
    }
}
