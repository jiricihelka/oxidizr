//! The optional `dagre` engine (`dagre` feature, OFF by default): [`Dagre`]
//! implements the same neutral [`LayoutEngine`] seam over the pure-Rust
//! `dagre` crate — a Sugiyama pipeline with network-simplex ranking,
//! Brandes-Köpf coordinate assignment, multigraph edges and compound
//! (nested) graph support. Enabling the feature adds only pure-computation
//! crates to the dependency tree (`dagre` + `log`, plus the `serde` pair the
//! engine's optional JSON reader needs — no IO, no windowing).
//!
//! Translation, in the order the seam needs it:
//!
//! * Domain ids never enter the engine: nodes are named `n<index>` and edges
//!   `e<index>` from their position in the input graph, so there is no
//!   escaping surface and re-emission is a positional lookup — the same
//!   convention the `Dot` engine uses. The edge name is what keeps the
//!   multigraph mode honest: parallel relationships stay distinct routes
//!   instead of collapsing into one.
//! * [`LayoutConfig`] maps onto the engine's options: the rank direction
//!   onto `rankdir`, the three separations onto `ranksep`/`nodesep`/`edgesep`
//!   (pixels, no unit conversion), and `marginx`/`marginy` pinned to 0 so the
//!   result is anchored at the origin like [`Layered`](crate::Layered)
//!   output. The ranker stays the crate's default (network simplex), which
//!   is also the quality choice.
//! * A node's reported `(x, y)` is the box CENTRE; the seam promises the
//!   TOP-LEFT, so the input half extents come off (input sizes, not the
//!   engine's — they are what the caller measured).
//! * Edge routes come back as polylines that already run source → target
//!   (edges reversed for cycle removal are undone and their points flipped)
//!   and are clipped to the box borders; the interior points are the
//!   waypoints. The pipeline splits long routes over dummy vertices and
//!   leaves a point on every rank border even when the route goes straight,
//!   so exactly-collinear interior points are dropped before handing the
//!   route over (interior means WAYPOINT — callers persist these). With
//!   `generate_vertices` off, only the two endpoints survive.
//! * Endpoint anchors ([`Anchor`](crate::Anchor)s) name the box side a
//!   route leaves. That pipeline has no port concept, so anchors are
//!   ignored here — [`Layered`](crate::Layered) is the engine that honours
//!   them.
//! * Long edges are split over inserted dummy vertices and edge labels get
//!   proxy nodes; both are internal bookkeeping and never re-emitted.
//!
//! Determinism: the engine's bookkeeping is insertion-ordered and
//! tree-map-based (no hash-order dependence, no randomness, no unstable
//! sorts), and its dummy-name counter is monotonic per process. The proof
//! lives in `tests/dagre_determinism.rs`: byte-identical output over a
//! fixture battery, repeated both inside one process (where that counter
//! drifts) and across fresh processes (where hash seeds differ). On top of
//! that, this adapter re-checks every coordinate: a missing label or a
//! non-finite placement is a [`LayoutError::Dagre`], never a `NaN` on its
//! way into written geometry.

use std::collections::BTreeMap;

use dagre::graph::{Graph, GraphOptions};
use dagre::layout::layout as run_dagre;
use dagre::{EdgeLabel, LayoutOptions, NodeLabel, RankDir};

use crate::engine::{EdgePlacement, LayoutEngine, LayoutError, LayoutResult, NodePlacement, Point};
use crate::graph::{LayoutConfig, LayoutGraph, RankDirection};

/// The node-name prefix: names are positional, never domain ids.
const NODE_PREFIX: char = 'n';

/// The edge-name prefix (the multigraph key that keeps parallel edges).
const EDGE_PREFIX: char = 'e';

/// Laid-out graph with the crate's label types.
type DagreGraph = Graph<NodeLabel, EdgeLabel>;

/// The `dagre` layered engine: stateless — every parameter comes from the
/// graph's [`LayoutConfig`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Dagre;

impl Dagre {
    /// The engine. Construction takes no configuration: the layout
    /// parameters travel with the graph.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl LayoutEngine for Dagre {
    /// Run the dagre pipeline over `graph` and translate the placements.
    /// An empty graph short-circuits (empty result, the convention every
    /// engine here follows — and the pipeline's bounding-box pass has
    /// nothing to anchor on).
    ///
    /// # Errors
    ///
    /// [`LayoutError`] from graph validation, or [`LayoutError::Dagre`]
    /// when the engine cannot place the graph.
    fn layout(&self, graph: &LayoutGraph) -> Result<LayoutResult, LayoutError> {
        graph.validate()?;
        if graph.nodes.is_empty() {
            return Ok(LayoutResult::default());
        }
        let mut laid = build(graph)?;
        run_dagre(&mut laid, Some(options(graph.config)));
        Ok(LayoutResult {
            nodes: node_placements(graph, &laid)?,
            edges: edge_placements(graph, &laid)?,
        })
    }
}

/// The engine options for `config`: direction and separations in pixels,
/// origin-anchored output, default ranker.
fn options(config: LayoutConfig) -> LayoutOptions {
    LayoutOptions {
        rankdir: rankdir(config.rank_direction),
        ranksep: f64::from(config.rank_separation),
        nodesep: f64::from(config.node_separation),
        edgesep: f64::from(config.edge_separation),
        marginx: 0.0,
        marginy: 0.0,
        ..LayoutOptions::default()
    }
}

/// The rank-direction mapping (names mirror the engine's four spellings).
const fn rankdir(direction: RankDirection) -> RankDir {
    match direction {
        RankDirection::TopDown => RankDir::TB,
        RankDirection::BottomUp => RankDir::BT,
        RankDirection::LeftRight => RankDir::LR,
        RankDirection::RightLeft => RankDir::RL,
    }
}

/// The node name for input position `index`.
fn node_name(index: usize) -> String {
    format!("{NODE_PREFIX}{index}")
}

/// The edge name for input position `index` (also the multigraph key).
fn edge_name(index: usize) -> String {
    format!("{EDGE_PREFIX}{index}")
}

/// The input graph in the engine's shape: directed, multigraph (parallel
/// relationships must survive), non-compound (boundary grouping is a caller
/// concern, added to the input graph, not to this translation).
fn build(graph: &LayoutGraph) -> Result<DagreGraph, LayoutError> {
    let mut laid = Graph::with_options(GraphOptions {
        directed: true,
        multigraph: true,
        compound: false,
    });
    for (index, node) in graph.nodes.iter().enumerate() {
        laid.set_node(
            node_name(index),
            Some(NodeLabel {
                width: node.width,
                height: node.height,
                ..NodeLabel::default()
            }),
        );
    }
    // Input position by id: lookups only, never iterated, so no map
    // ordering can reach the output.
    let mut positions: BTreeMap<&str, usize> = BTreeMap::new();
    for (index, node) in graph.nodes.iter().enumerate() {
        positions.insert(node.id.as_str(), index);
    }
    for (index, edge) in graph.edges.iter().enumerate() {
        // Unreachable after `validate` (it resolves every endpoint);
        // naming a missing one loudly beats inventing a phantom node.
        let at = |id: &crate::NodeId| {
            positions
                .get(id.as_str())
                .copied()
                .ok_or_else(|| reject(format!("edge {} names unknown node {id}", edge.id)))
                .map(node_name)
        };
        laid.set_edge(
            at(&edge.source)?,
            at(&edge.target)?,
            Some(EdgeLabel::default()),
            Some(edge_name(index).as_str()),
        );
    }
    Ok(laid)
}

/// Node placements in input order: centre minus the input half extents.
fn node_placements(
    graph: &LayoutGraph,
    laid: &DagreGraph,
) -> Result<Vec<NodePlacement>, LayoutError> {
    graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let label = laid
                .node(&node_name(index))
                .ok_or_else(|| reject(format!("node {} is missing", node.id)))?;
            let (Some(x), Some(y)) = (label.x, label.y) else {
                return Err(reject(format!("node {} is unplaced", node.id)));
            };
            if !(x.is_finite() && y.is_finite()) {
                return Err(reject(format!("node {} is not finite", node.id)));
            }
            Ok(NodePlacement {
                id: node.id.clone(),
                position: Point {
                    x: x - node.width / 2.0,
                    y: y - node.height / 2.0,
                },
            })
        })
        .collect()
}

/// Edge routes in input order: the pipeline's polylines keyed by the
/// positional edge name (route order is already source → target).
fn edge_placements(
    graph: &LayoutGraph,
    laid: &DagreGraph,
) -> Result<Vec<EdgePlacement>, LayoutError> {
    // Every edge the pipeline still knows, with its route. Real edges are
    // restored to their original endpoints at the end of the run; internal
    // leftovers simply carry names this input never gave, so they are
    // ignored by the lookups below.
    let mut routes: BTreeMap<String, Vec<Point>> = BTreeMap::new();
    for edge in laid.edges() {
        if let Some(name) = edge.name.as_deref()
            && let Some(label) = laid.edge(&edge.v, &edge.w, edge.name.as_deref())
        {
            routes.insert(
                name.to_owned(),
                label
                    .points
                    .iter()
                    .map(|p| Point { x: p.x, y: p.y })
                    .collect(),
            );
        }
    }
    graph
        .edges
        .iter()
        .enumerate()
        .map(|(index, edge)| {
            let route = routes
                .get(&edge_name(index))
                .ok_or_else(|| reject(format!("edge {} is unrouted", edge.id)))?;
            if route.iter().any(|p| !(p.x.is_finite() && p.y.is_finite())) {
                return Err(reject(format!("edge {} has a non-finite point", edge.id)));
            }
            let route = simplify(route);
            let points = if graph.config.generate_vertices {
                route
            } else {
                route
                    .first()
                    .into_iter()
                    .chain(route.last())
                    .copied()
                    .collect()
            };
            Ok(EdgePlacement {
                id: edge.id.clone(),
                points,
            })
        })
        .collect()
}

/// Drop interior points that continue the previous segment exactly
/// straight. The pipeline splits long routes over dummy vertices, so a
/// straight route comes back with one point per rank border — collinear
/// points that are not waypoints. The seam defines interior points AS
/// waypoints (and callers persist them), so simplification removes only
/// EXACT collinearity (an `f64` cross product of zero, no epsilon): the
/// route never moves and the result stays deterministic.
fn simplify(route: &[Point]) -> Vec<Point> {
    let Some((&first, rest)) = route.split_first() else {
        return Vec::new();
    };
    let mut out = vec![first];
    for &point in rest {
        // A point that only continues the current leg replaces the previous
        // one (the leg keeps its direction and gains the further reach).
        if out.len() >= 2 {
            let one = out[out.len() - 2];
            let two = out[out.len() - 1];
            let straight =
                (two.x - one.x) * (point.y - two.y) - (two.y - one.y) * (point.x - two.x);
            if straight == 0.0 {
                out.pop();
            }
        }
        out.push(point);
    }
    out
}

/// The engine-side failure shape (the graph itself is valid — the pipeline
/// could not place it).
fn reject(message: String) -> LayoutError {
    LayoutError::Dagre(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Edge, EdgeId, Node, NodeId};

    fn chain(direction: RankDirection) -> LayoutGraph {
        let mut g = LayoutGraph::new();
        g.config.rank_direction = direction;
        for id in ["a", "b", "c"] {
            g.add_node(Node::new(NodeId::new(id), 120.0, 60.0));
        }
        g.add_edge(Edge::new(
            EdgeId::new("ab"),
            NodeId::new("a"),
            NodeId::new("b"),
        ));
        g.add_edge(Edge::new(
            EdgeId::new("bc"),
            NodeId::new("b"),
            NodeId::new("c"),
        ));
        g
    }

    fn place(result: &LayoutResult, id: &str) -> Point {
        result
            .nodes
            .iter()
            .find(|n| n.id.as_str() == id)
            .map_or_else(|| panic!("{id} missing from the result"), |n| n.position)
    }

    #[test]
    fn empty_graph_short_circuits() {
        let result = Dagre::new().layout(&LayoutGraph::new()).expect("empty");
        assert_eq!(result, LayoutResult::default());
    }

    #[test]
    fn invalid_graph_is_rejected_before_the_pipeline() {
        let mut g = LayoutGraph::new();
        g.add_node(Node::new(NodeId::new("a"), 10.0, 10.0));
        g.add_edge(Edge::new(
            EdgeId::new("r"),
            NodeId::new("a"),
            NodeId::new("ghost"),
        ));
        assert!(matches!(
            Dagre::new().layout(&g),
            Err(LayoutError::UnknownEdgeEndpoint { .. })
        ));
    }

    #[test]
    fn results_are_input_ordered_and_complete() {
        let result = Dagre::new()
            .layout(&chain(RankDirection::LeftRight))
            .expect("lr");
        let ids: Vec<&str> = result.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c"]);
        let edges: Vec<&str> = result.edges.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(edges, ["ab", "bc"]);
        assert!(result.edges.iter().all(|e| e.points.len() >= 2));
    }

    #[test]
    fn rank_direction_orders_the_chain() {
        for direction in [
            RankDirection::LeftRight,
            RankDirection::RightLeft,
            RankDirection::TopDown,
            RankDirection::BottomUp,
        ] {
            let result = Dagre::new().layout(&chain(direction)).expect("chain");
            let axis = |p: Point| match direction {
                RankDirection::LeftRight | RankDirection::RightLeft => p.x,
                RankDirection::TopDown | RankDirection::BottomUp => p.y,
            };
            let (a, b, c) = (
                axis(place(&result, "a")),
                axis(place(&result, "b")),
                axis(place(&result, "c")),
            );
            let ordered = match direction {
                RankDirection::LeftRight | RankDirection::TopDown => a < b && b < c,
                RankDirection::RightLeft | RankDirection::BottomUp => a > b && b > c,
            };
            assert!(ordered, "{direction:?} ranks out of order: {result:?}");
        }
    }

    #[test]
    fn boxes_do_not_overlap() {
        let result = Dagre::new()
            .layout(&chain(RankDirection::TopDown))
            .expect("tb");
        let boxes: Vec<Point> = result.nodes.iter().map(|n| n.position).collect();
        for (i, one) in boxes.iter().enumerate() {
            for two in boxes.iter().skip(i + 1) {
                let horizontal = one.x + 120.0 <= two.x || two.x + 120.0 <= one.x;
                let vertical = one.y + 60.0 <= two.y || two.y + 60.0 <= one.y;
                assert!(horizontal || vertical, "overlap: {one:?} vs {two:?}");
            }
        }
    }

    #[test]
    fn routes_run_source_to_target_and_start_on_the_border() {
        let mut g = LayoutGraph::new();
        g.config.rank_direction = RankDirection::LeftRight;
        g.add_node(Node::new(NodeId::new("a"), 100.0, 40.0));
        g.add_node(Node::new(NodeId::new("b"), 100.0, 40.0));
        g.add_edge(Edge::new(
            EdgeId::new("ab"),
            NodeId::new("a"),
            NodeId::new("b"),
        ));
        let result = Dagre::new().layout(&g).expect("two nodes");
        let source = result.nodes[0].position;
        let target = result.nodes[1].position;
        let route = &result.edges[0].points;
        let first = route.first().expect("first point");
        let last = route.last().expect("last point");
        // Clipped to the boxes: the route leaves the source's right border
        // and enters the target's left border (left-right ranks).
        assert!(
            (first.x - (source.x + 100.0)).abs() < 1.0,
            "route starts at {first:?}, source right edge is {}",
            source.x + 100.0
        );
        assert!(
            (last.x - target.x).abs() < 1.0,
            "route ends at {last:?}, target left edge is {}",
            target.x
        );
    }

    #[test]
    fn parallel_edges_and_self_loops_route() {
        let mut g = LayoutGraph::new();
        g.add_node(Node::new(NodeId::new("a"), 120.0, 60.0));
        g.add_node(Node::new(NodeId::new("b"), 120.0, 60.0));
        // Two parallel relationships plus a self-loop — shapes a real C4
        // view produces.
        for (index, (from, to)) in [("a", "b"), ("a", "b"), ("a", "a")].into_iter().enumerate() {
            g.add_edge(Edge::new(
                EdgeId::new(format!("r{index}")),
                NodeId::new(from),
                NodeId::new(to),
            ));
        }
        let result = Dagre::new().layout(&g).expect("multi");
        assert_eq!(result.edges.len(), 3);
        for edge in &result.edges {
            assert!(edge.points.len() >= 2, "{} unrouted", edge.id);
        }
    }

    #[test]
    fn vertices_off_keeps_only_the_endpoints() {
        let mut g = chain(RankDirection::TopDown);
        g.add_edge(Edge::new(
            EdgeId::new("ac"),
            NodeId::new("a"),
            NodeId::new("c"),
        ));
        let with = Dagre::new().layout(&g).expect("with vertices");
        let longest = with
            .edges
            .iter()
            .map(|e| e.points.len())
            .max()
            .unwrap_or_default();
        g.config.generate_vertices = false;
        let without = Dagre::new().layout(&g).expect("no vertices");
        assert!(longest > 2, "fixture should produce a bent route");
        assert!(
            without.edges.iter().all(|e| e.points.len() <= 2),
            "interior points survived"
        );
    }

    #[test]
    fn zero_separations_stay_finite() {
        let mut g = chain(RankDirection::LeftRight);
        g.config.rank_separation = 0;
        g.config.node_separation = 0;
        g.config.edge_separation = 0;
        let result = Dagre::new().layout(&g).expect("tight");
        assert!(
            result
                .nodes
                .iter()
                .all(|n| n.position.x.is_finite() && n.position.y.is_finite()),
            "degenerate separations produced a non-finite placement"
        );
    }

    #[test]
    fn straight_routes_keep_no_interior_points() {
        // The pipeline leaves one point per rank border even on a straight
        // route; the seam persists interiors as waypoints, so a straight
        // chain must come back with bare endpoints.
        let result = Dagre::new()
            .layout(&chain(RankDirection::LeftRight))
            .expect("chain");
        for edge in &result.edges {
            assert_eq!(
                edge.points.len(),
                2,
                "route {} keeps a collinear interior: {:?}",
                edge.id,
                edge.points
            );
        }
    }

    #[test]
    fn simplify_keeps_bends_and_drops_straights() {
        let point = |x: f64, y: f64| Point { x, y };
        // Straight with two border ticks → endpoints only.
        let straight = vec![
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(2.0, 0.0),
            point(3.0, 0.0),
        ];
        assert_eq!(simplify(&straight), vec![point(0.0, 0.0), point(3.0, 0.0)]);
        // A real bend survives untouched.
        let bent = vec![point(0.0, 0.0), point(1.0, 0.0), point(1.0, 1.0)];
        assert_eq!(simplify(&bent), bent);
        // A bend whose NEXT leg runs on through the bend point: the bend
        // stays (it is a true corner), the trailing tick goes.
        let corner = vec![
            point(0.0, 0.0),
            point(0.0, 1.0),
            point(0.0, 2.0),
            point(1.0, 2.0),
        ];
        assert_eq!(
            simplify(&corner),
            vec![point(0.0, 0.0), point(0.0, 2.0), point(1.0, 2.0)]
        );
        let empty: Vec<Point> = Vec::new();
        assert_eq!(simplify(&empty).len(), 0);
    }

    #[test]
    fn cycles_and_disconnected_components_place() {
        let mut g = LayoutGraph::new();
        for id in ["a", "b", "c", "lonely"] {
            g.add_node(Node::new(NodeId::new(id), 80.0, 40.0));
        }
        for (index, (from, to)) in [("a", "b"), ("b", "c"), ("c", "a")].into_iter().enumerate() {
            g.add_edge(Edge::new(
                EdgeId::new(format!("r{index}")),
                NodeId::new(from),
                NodeId::new(to),
            ));
        }
        let result = Dagre::new().layout(&g).expect("cycle + island");
        assert_eq!(result.nodes.len(), 4);
        assert_eq!(result.edges.len(), 3);
        assert!(result.nodes.iter().all(|n| n.position.x.is_finite()));
    }
}
