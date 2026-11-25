//! The optional graphviz `dot` backend (`dot-backend` feature, OFF by
//! default): [`Dot`] implements the same neutral [`LayoutEngine`] seam by
//! shelling out to `$OXIDZR_DOT_BIN` with `-Tplain` and parsing the
//! placements back. The real-binary test only runs when
//! `$OXIDZR_DOT_BIN` is set.
//!
//! `dot -Tplain` format facts:
//!
//! - Plain output is in INCHES (72 pt/in ⇒ we use 72 px/in), the y-axis
//!   points UP, and the origin is the bottom-left of each connected
//!   component.
//! - Each connected component prints its own `graph G w h` block with
//!   local coordinates — [`Dot::parse_plain`] stacks components
//!   left-to-right with [`COMPONENT_GAP_PX`] between them.
//! - `node` rows are `node NAME x y w h …` with `(x, y)` the box
//!   CENTRE and `w`/`h` the FULL box extents (the declared fixedsize
//!   inches), so the top-left is `(x-w/2, graph_h-y-h/2)` in our frame.
//! - `edge` rows are `edge SRC DST N p1 p2 …`; a point is either the
//!   documented joined `x,y` spelling or two plain numbers — both are
//!   parsed. With `splines=true` the points trace the routed polyline,
//!   with `splines=false` exactly the two endpoints.
//!
//! Determinism: the DOT source is written in graph input order
//! with positional names (`n0`, `n1`, …), results are re-emitted in
//! input order, and plain output is deterministic for a fixed graphviz
//! build — so same input + same binary ⇒ identical output. Parallel
//! edges (the same ordered pair twice) cannot be told apart by NAME in
//! plain output; they are matched per pair in output order (FIFO),
//! which graphviz emits consistently.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::engine::{EdgePlacement, LayoutEngine, LayoutError, LayoutResult, NodePlacement, Point};
use crate::graph::{Anchor, LayoutGraph};

/// Plain-format unit: 72 pt/in — we treat 1 in = 72 px.
pub(crate) const PX_PER_INCH: f64 = 72.0;
/// Horizontal gap between stacked connected components (px).
pub(crate) const COMPONENT_GAP_PX: f64 = 50.0;

/// The external-`dot` engine: holds the binary path (discovery via
/// [`Dot::from_env`] from `$OXIDZR_DOT_BIN`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dot {
    binary: PathBuf,
}

impl Dot {
    /// Engine invoking `binary` directly.
    #[must_use]
    pub const fn new(binary: PathBuf) -> Self {
        Self { binary }
    }

    /// The binary path this engine invokes.
    #[must_use]
    pub fn binary(&self) -> &PathBuf {
        &self.binary
    }

    /// `$OXIDZR_DOT_BIN` discovery (empty/unset ⇒ `None`).
    #[must_use]
    pub fn from_env() -> Option<Self> {
        let bin = std::env::var_os("OXIDZR_DOT_BIN")?;
        (!bin.is_empty()).then(|| Self::new(PathBuf::from(bin)))
    }

    /// The DOT source for `graph`: input order, positional names, sizes
    /// converted to inches, no labels (so `fixedsize` boxes are exact).
    /// Public for the round-trip tests (source ⇄ fixture).
    #[must_use]
    pub fn dot_source(graph: &LayoutGraph) -> String {
        let mut out = String::from("digraph {\n");
        let _ = writeln!(
            out,
            "  graph [rankdir={}, nodesep={:.4}, ranksep={:.4}, esep={:.4}, \
             overlap=false, splines={}];",
            rankdir(graph.config.rank_direction),
            f64::from(graph.config.node_separation) / PX_PER_INCH,
            f64::from(graph.config.rank_separation) / PX_PER_INCH,
            f64::from(graph.config.edge_separation) / PX_PER_INCH,
            if graph.config.generate_vertices {
                "true"
            } else {
                "false"
            },
        );
        for (index, node) in graph.nodes.iter().enumerate() {
            let _ = writeln!(
                out,
                "  n{index} [label=\"\", shape=box, fixedsize=true, width={:.4}, \
                 height={:.4}];",
                node.width / PX_PER_INCH,
                node.height / PX_PER_INCH,
            );
        }
        for edge in &graph.edges {
            // Ids map to input-order positions — ids themselves never
            // enter the DOT source (no escaping surface at all).
            let source = node_index(graph, &edge.source);
            let target = node_index(graph, &edge.target);
            let _ = write!(out, "  n{source} -> n{target}[");
            let mut attrs = Vec::new();
            if let Some(port) = edge.from.map(port_name) {
                attrs.push(format!("tailport={port}"));
            }
            if let Some(port) = edge.to.map(port_name) {
                attrs.push(format!("headport={port}"));
            }
            out.push_str(&attrs.join(","));
            out.push_str("];\n");
        }
        out.push_str("}\n");
        out
    }

    /// Parse `-Tplain` output for `graph` into placements (input order,
    /// y-flip, component stacking — module docs). `edge_count` must
    /// match per source→target pair or the output is rejected as
    /// inconsistent.
    ///
    /// # Errors
    ///
    /// [`LayoutError::DotBackend`] when a row is malformed, a graphviz
    /// node name is out of range, or edge rows disagree with the graph.
    // One flat line-loop over the row kinds is the clearest shape of
    // this parser — splitting it would only scatter the shared
    // component-stack state.
    #[allow(clippy::too_many_lines)]
    pub fn parse_plain(graph: &LayoutGraph, plain: &str) -> Result<LayoutResult, LayoutError> {
        let fail = |msg: String| LayoutError::DotBackend(msg);
        let mut nodes = vec![None; graph.nodes.len()];
        // Edge routes keyed by (source index, target index), consumed
        // FIFO per pair (parallel edges share the key, module docs).
        let mut routes: std::collections::BTreeMap<(usize, usize), Vec<Vec<Point>>> =
            std::collections::BTreeMap::new();
        let mut origin_x = 0.0_f64;
        let mut graph_h = 0.0_f64;
        let mut max_right = 0.0_f64;
        let mut seen_component = false;
        for line in plain.lines() {
            let mut words = line.split_whitespace();
            match words.next() {
                Some("graph") => {
                    // New connected component: the previous one ends.
                    // The FIRST block starts at the global origin —
                    // the gap only goes BETWEEN stacked components.
                    if seen_component {
                        origin_x = max_right + COMPONENT_GAP_PX;
                    }
                    seen_component = true;
                    let _id = words.next();
                    let w = next_f64(&mut words, "graph width")?;
                    graph_h = next_f64(&mut words, "graph height")?;
                    max_right = (origin_x + w * PX_PER_INCH).max(max_right);
                }
                Some("node") => {
                    let name = words.next().unwrap_or_default();
                    let index = name
                        .strip_prefix('n')
                        .and_then(|i| i.parse::<usize>().ok())
                        .filter(|i| *i < nodes.len())
                        .ok_or_else(|| fail(format!("plain: unknown node {name:?}")))?;
                    let mut num = |what: &str| -> Result<f64, LayoutError> {
                        next_f64(&mut words, &format!("{what} of {name}"))
                    };
                    let cx = num("x")?;
                    let cy = num("y")?;
                    // `w`/`h` are the FULL box extents (they echo
                    // the declared fixedsize inches), so the
                    // half-extent is half.
                    let half_w = num("width")? / 2.0;
                    let half_h = num("height")? / 2.0;
                    // Plain y is bottom-up; top-left in this component's
                    // frame, then shifted by the stacked origin.
                    nodes[index] = Some(Point {
                        x: (cx - half_w) * PX_PER_INCH + origin_x,
                        y: (graph_h - cy - half_h) * PX_PER_INCH,
                    });
                }
                Some("edge") => {
                    let src = words.next().unwrap_or_default();
                    let dst = words.next().unwrap_or_default();
                    let count: usize = words
                        .next()
                        .and_then(|c| c.parse().ok())
                        .ok_or_else(|| fail("plain: bad edge point count".to_owned()))?;
                    let mut points = Vec::with_capacity(count);
                    for _ in 0..count {
                        // A point is either one joined `x,y` token
                        // or two plain numbers — accept both shapes.
                        let first = words
                            .next()
                            .ok_or_else(|| fail("plain: truncated edge points".to_owned()))?;
                        let (xs, ys) = match first.split_once(',') {
                            Some((a, b)) => (a, b),
                            None => (
                                first,
                                words.next().ok_or_else(|| {
                                    fail("plain: truncated edge points".to_owned())
                                })?,
                            ),
                        };
                        let x = xs
                            .parse::<f64>()
                            .map_err(|_| fail(format!("plain: bad edge x {xs:?}")))?;
                        let y = ys
                            .parse::<f64>()
                            .map_err(|_| fail(format!("plain: bad edge y {ys:?}")))?;
                        points.push(Point {
                            x: x * PX_PER_INCH + origin_x,
                            y: (graph_h - y) * PX_PER_INCH,
                        });
                    }
                    let key = (self_pos(graph, src)?, self_pos(graph, dst)?);
                    routes.entry(key).or_default().push(points);
                }
                // `stop`, the stream end, and any future row kinds:
                // nothing to place, ignore.
                _ => {}
            }
        }
        let placed = nodes
            .iter()
            .enumerate()
            .map(|(index, p)| {
                p.map(|position| NodePlacement {
                    id: graph.nodes[index].id.clone(),
                    position,
                })
                .ok_or_else(|| {
                    fail(format!(
                        "plain: node n{index} ({}) missing",
                        graph.nodes[index].id
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut edges = Vec::with_capacity(graph.edges.len());
        for edge in &graph.edges {
            let key = (
                node_index(graph, &edge.source),
                node_index(graph, &edge.target),
            );
            let points = routes
                .get_mut(&key)
                .and_then(|list| (!list.is_empty()).then(|| list.remove(0)))
                .ok_or_else(|| {
                    fail(format!(
                        "plain: no route for edge {} ({} -> {})",
                        edge.id, edge.source, edge.target
                    ))
                })?;
            let keep = if graph.config.generate_vertices {
                points
            } else {
                let mut ends = Vec::new();
                if let Some(first) = points.first() {
                    ends.push(*first);
                }
                if let Some(last) = points.last() {
                    ends.push(*last);
                }
                ends
            };
            edges.push(EdgePlacement {
                id: edge.id.clone(),
                points: keep,
            });
        }
        Ok(LayoutResult {
            nodes: placed,
            edges,
        })
    }
}

impl LayoutEngine for Dot {
    /// Run `<binary> -Tplain` with [`Self::dot_source`] on stdin and
    /// parse the placements. An empty graph never spawns a process
    /// (empty result, the [`Layered`](crate::Layered) convention).
    ///
    /// # Errors
    ///
    /// [`LayoutError`] from graph validation, spawn/exit failure or
    /// unparseable plain output (all as [`LayoutError::DotBackend`]).
    fn layout(&self, graph: &LayoutGraph) -> Result<LayoutResult, LayoutError> {
        graph.validate()?;
        if graph.nodes.is_empty() {
            return Ok(LayoutResult::default());
        }
        let mut child = Command::new(&self.binary)
            .arg("-Tplain")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                LayoutError::DotBackend(format!("spawn {}: {e}", self.binary.display()))
            })?;
        child
            .stdin
            .as_mut()
            .map(|stdin| stdin.write_all(Self::dot_source(graph).as_bytes()))
            .transpose()
            .map_err(|e| LayoutError::DotBackend(format!("pipe dot input: {e}")))?;
        let output = child
            .wait_with_output()
            .map_err(|e| LayoutError::DotBackend(format!("run dot: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(LayoutError::DotBackend(format!(
                "dot exited with {}: {}",
                output.status,
                stderr.trim()
            )));
        }
        let plain = String::from_utf8_lossy(&output.stdout);
        Self::parse_plain(graph, &plain)
    }
}

/// Parse the next whitespace token as a plain-format f64.
fn next_f64(words: &mut std::str::SplitWhitespace<'_>, what: &str) -> Result<f64, LayoutError> {
    words
        .next()
        .and_then(|v| v.parse::<f64>().ok())
        .ok_or_else(|| LayoutError::DotBackend(format!("plain: bad {what}")))
}

/// `rankdir` for the four rank directions.
const fn rankdir(direction: crate::RankDirection) -> &'static str {
    use crate::RankDirection::{BottomUp, LeftRight, RightLeft, TopDown};
    match direction {
        TopDown => "TB",
        BottomUp => "BT",
        LeftRight => "LR",
        RightLeft => "RL",
    }
}

/// The `*port` name for a box side (graphviz compass points).
const fn port_name(anchor: Anchor) -> &'static str {
    match anchor {
        Anchor::Top => "n",
        Anchor::Right => "e",
        Anchor::Bottom => "s",
        Anchor::Left => "w",
    }
}

/// Input-order index of a (validated-unique) node id.
fn node_index(graph: &LayoutGraph, id: &crate::NodeId) -> usize {
    graph
        .nodes
        .iter()
        .position(|n| n.id == *id)
        .unwrap_or(graph.nodes.len())
}

/// Parse a `n{index}` plain-output name to its index.
fn self_pos(graph: &LayoutGraph, name: &str) -> Result<usize, LayoutError> {
    name.strip_prefix('n')
        .and_then(|i| i.parse::<usize>().ok())
        .filter(|i| *i < graph.nodes.len())
        .ok_or_else(|| LayoutError::DotBackend(format!("plain: unknown node {name:?}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Edge, EdgeId, Node, NodeId};

    fn chain() -> LayoutGraph {
        let mut g = LayoutGraph::new();
        g.config.rank_direction = crate::RankDirection::LeftRight;
        g.add_node(Node::new(NodeId::new("a"), 350.0, 150.0));
        g.add_node(Node::new(NodeId::new("b"), 350.0, 150.0));
        g.add_edge(Edge::new(
            EdgeId::new("e"),
            NodeId::new("a"),
            NodeId::new("b"),
        ));
        g
    }

    #[test]
    fn dot_source_is_positional_and_ordered() {
        let src = Dot::dot_source(&chain());
        assert!(src.contains("rankdir=LR"));
        // 350 px = 4.8611 in (72 px/in).
        assert!(
            src.contains("n0 [label=\"\", shape=box, fixedsize=true, width=4.8611"),
            "{src}"
        );
        assert!(src.contains("n0 -> n1"), "{src}");
        assert!(!src.contains("\"a\""), "ids never enter the source: {src}");
    }

    #[test]
    fn parse_plain_flips_stacks_and_maps_order() {
        // One component 10x4 in: n0 centred (1,2), FULL extents 1x2
        // (the real row shape).
        let plain = "graph 1 10 4\n\
                     node n0 1 2 1 2 solid box black lightgrey \"\"\n\
                     node n1 9 2 1 2 solid box black lightgrey \"\"\n\
                     edge n0 n1 2 1.5,2 8.5,2\n\
                     stop\n";
        let result = Dot::parse_plain(&chain(), plain).expect("plain parses");
        assert_eq!(result.nodes.len(), 2);
        // Top-left = (x-w/2, graph_h-y-h/2) in ×72.
        assert_eq!(result.nodes[0].position, Point { x: 36.0, y: 72.0 });
        assert_eq!(result.nodes[1].position, Point { x: 612.0, y: 72.0 });
        assert_eq!(result.edges[0].points.len(), 2);
        assert_eq!(result.edges[0].points[0], Point { x: 108.0, y: 144.0 });
        // Input order, ids from the graph.
        assert_eq!(result.nodes[0].id, NodeId::new("a"));
    }

    #[test]
    fn parse_plain_stacks_components() {
        let mut g = chain();
        g.edges.clear();
        g.add_node(Node::new(NodeId::new("c"), 72.0, 72.0));
        let plain = "graph 1 10 4\n\
                     node n0 1 2 1 2 solid box black lightgrey \"\"\n\
                     node n1 3 2 1 2 solid box black lightgrey \"\"\n\
                     graph 2 4 2\n\
                     node n2 1 1 1 1 solid box black lightgrey \"\"\n\
                     stop\n";
        let result = Dot::parse_plain(&g, plain).expect("two components");
        // Second component starts after 10 in + gap.
        assert!(
            (result.nodes[2].position.x - (720.0 + COMPONENT_GAP_PX + 36.0)).abs() < 1e-9,
            "stacked: {:?}",
            result.nodes[2].position
        );
    }

    #[test]
    fn straight_mode_keeps_only_endpoints() {
        let mut g = chain();
        g.config.generate_vertices = false;
        let plain = "graph 1 10 4\n\
                     node n0 1 2 1 2 solid box black lightgrey \"\"\n\
                     node n1 9 2 1 2 solid box black lightgrey \"\"\n\
                     edge n0 n1 4 1.5,2 3,1 7,3 8.5,2\n\
                     stop\n";
        let result = Dot::parse_plain(&g, plain).expect("plain parses");
        assert_eq!(result.edges[0].points.len(), 2);
        assert_eq!(result.edges[0].points[1], Point { x: 612.0, y: 144.0 });
    }

    #[test]
    fn malformed_plain_is_an_error_value() {
        assert!(matches!(
            Dot::parse_plain(&chain(), "graph 1 x y\nnode n0 1 1 1 1 e\nstop\n"),
            Err(LayoutError::DotBackend(_))
        ));
        assert!(
            matches!(
                Dot::parse_plain(&chain(), "stop\n"),
                Err(LayoutError::DotBackend(_))
            ),
            "missing node rows are caught"
        );
    }

    #[test]
    fn empty_graph_never_spawns() {
        let engine = Dot::new(PathBuf::from("/nonexistent/dot"));
        assert_eq!(
            engine.layout(&LayoutGraph::new()).expect("empty"),
            LayoutResult::default()
        );
    }

    /// Real-binary test, gated on `$OXIDZR_DOT_BIN` — skipped
    /// silently when the var is unset. Pins the contract: one
    /// placement per node in input order, LR ranks left→right, routes
    /// have endpoints, and TWO runs are identical (determinism).
    #[test]
    fn real_dot_binary_places_and_is_deterministic() {
        let Some(engine) = Dot::from_env() else {
            eprintln!("OXIDZR_DOT_BIN unset — skipping Category B dot test");
            return;
        };
        let first = engine.layout(&chain()).expect("dot runs");
        let second = engine.layout(&chain()).expect("dot runs again");
        assert_eq!(first, second, "determinism contract");
        assert_eq!(first.nodes.len(), 2);
        assert_eq!(first.nodes[0].id, NodeId::new("a"));
        assert_eq!(first.nodes[1].id, NodeId::new("b"));
        assert!(
            first.nodes[1].position.x > first.nodes[0].position.x,
            "LR chain ranks left to right: {:?}",
            first.nodes
        );
        assert!(first.edges[0].points.len() >= 2);
    }
}
