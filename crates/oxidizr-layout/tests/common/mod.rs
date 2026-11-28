//! Shared test corpus for the layout engines: fixture shapes a real C4 view
//! produces, the variant sweep over the layout configuration, and the
//! canonical byte encoding the determinism proof compares. Test-only —
//! nothing here ships.
//!
//! Every fixture is deterministic by construction: the pseudo-random ones
//! run their own fixed-seed generator (no system entropy, no clock), so the
//! same corpus is built in every process.

#![allow(dead_code)] // each test file uses a subset

use oxidizr_layout::{
    Edge, EdgeId, LayoutEngine, LayoutGraph, LayoutResult, Node, NodeId, RankDirection,
};

/// One named graph shape.
pub struct Fixture {
    /// Stable identifier (the report/metric key).
    pub name: &'static str,
    /// Builds the graph, fresh each call.
    pub build: fn() -> LayoutGraph,
    /// How wide the configuration sweep runs for this shape.
    pub sweep: Sweep,
}

/// How wide a fixture enters the configuration sweep. The small shapes carry
/// the full sweep (they cost microseconds); the big ones only the four rank
/// directions, so a proof battery stays a second-scale operation while still
/// hashing thousands of coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sweep {
    /// Four directions + tight separations + vertices-off.
    Full,
    /// Four directions, contract separations.
    Directions,
}

/// The corpus: `name` + builder pairs, in a fixed order.
pub fn corpus() -> Vec<Fixture> {
    let full = |name: &'static str, build: fn() -> LayoutGraph| Fixture {
        name,
        build,
        sweep: Sweep::Full,
    };
    vec![
        full("single", single),
        full("chain-8", || chain(8)),
        full("diamond", diamond),
        full("fan-out", fan_out),
        full("fan-in", fan_in),
        full("cycle", cycle),
        full("self-loops", self_loops),
        full("parallel", parallel),
        full("disconnected", disconnected),
        full("mixed-sizes", mixed_sizes),
        full("odd-ids", odd_ids),
        full("c4-like", c4_like),
        Fixture {
            name: "tangled-90",
            build: || tangled(90, 170),
            sweep: Sweep::Directions,
        },
        Fixture {
            name: "grid-120",
            build: || layered_grid(10, 12),
            sweep: Sweep::Directions,
        },
    ]
}

/// The four rank directions, in contract order.
pub fn directions() -> [RankDirection; 4] {
    [
        RankDirection::TopDown,
        RankDirection::BottomUp,
        RankDirection::LeftRight,
        RankDirection::RightLeft,
    ]
}

/// The configuration sweep for one fixture: every rank direction under the
/// contract's default separations, and — for `Sweep::Full` shapes — a
/// tight-separation profile (small rank, zero node/edge gaps, the degenerate
/// corner) plus a vertices-off profile. The pair list is
/// `(variant key, graph)`, in a fixed order.
pub fn variants(fixture: &Fixture) -> Vec<(String, LayoutGraph)> {
    let mut out = Vec::new();
    for direction in directions() {
        let mut g = (fixture.build)();
        g.config.rank_direction = direction;
        out.push((format!("{}/{}", fixture.name, tag(direction)), g));
    }
    if fixture.sweep == Sweep::Directions {
        return out;
    }
    for direction in directions() {
        let mut g = (fixture.build)();
        g.config.rank_direction = direction;
        g.config.rank_separation = 12;
        g.config.node_separation = 0;
        g.config.edge_separation = 0;
        out.push((format!("{}/{}/tight", fixture.name, tag(direction)), g));
    }
    let mut g = (fixture.build)();
    g.config.generate_vertices = false;
    out.push((format!("{}/flat", fixture.name), g));
    out
}

/// Short stable spelling of a rank direction (report keys).
pub fn tag(direction: RankDirection) -> &'static str {
    match direction {
        RankDirection::TopDown => "tb",
        RankDirection::BottomUp => "bt",
        RankDirection::LeftRight => "lr",
        RankDirection::RightLeft => "rl",
    }
}

/// Lays `graph` out with `engine` and encodes the result canonically.
///
/// # Errors
///
/// Whatever the engine reports (a fixture the engine cannot place is a test
/// failure, so the caller decides how to present it).
pub fn encode_layout(
    engine: &dyn LayoutEngine,
    graph: &LayoutGraph,
) -> Result<String, oxidizr_layout::LayoutError> {
    engine.layout(graph).map(|result| encode(&result))
}

/// Canonical text for a result: ids plus bit-exact coordinates, so the
/// comparison is byte-level and formatting can never hide a difference.
pub fn encode(result: &LayoutResult) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for node in &result.nodes {
        let _ = writeln!(
            out,
            "N {} {}\t{}",
            node.id,
            bits(node.position.x),
            bits(node.position.y)
        );
    }
    for edge in &result.edges {
        let _ = write!(out, "E {}", edge.id);
        for point in &edge.points {
            let _ = write!(out, " {} {}", bits(point.x), bits(point.y));
        }
        let _ = writeln!(out);
    }
    out
}

/// Bit pattern of a coordinate (hex, fixed width) — `NaN`/`±0.0` included.
pub fn bits(value: f64) -> String {
    format!("{:016x}", value.to_bits())
}

/// FNV-1a 64-bit digest of `text` (report keys, no hashing crate).
pub fn digest(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// The full battery report: one `key digest` line per fixture variant.
/// `engine` is named only for the header of the report.
///
/// Panics if an engine fails a fixture: every shape here must be placeable.
pub fn report(engine: &dyn LayoutEngine, engine_name: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let _ = writeln!(out, "# engine {engine_name}");
    for fixture in corpus() {
        for (key, graph) in variants(&fixture) {
            let encoded = encode_layout(engine, &graph)
                .unwrap_or_else(|error| panic!("{key}: engine failed: {error}"));
            let _ = writeln!(out, "{key} {}", digest(&encoded));
        }
    }
    out
}

// --- fixture builders -------------------------------------------------------

/// Fixed-seed generator (no entropy, no clock): `mulberry32` over a `u64`.
pub struct Lcg(u64);

impl Lcg {
    /// A generator at a fixed seed — the same stream in every process.
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The next pseudo-random value in `[0, bound)`.
    pub fn below(&mut self, bound: u64) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let mixed = (self.0 >> 32) ^ (self.0 >> 16);
        usize::try_from(mixed % bound).unwrap_or(usize::MAX)
    }
}

fn node(id: &str, width: f64, height: f64) -> Node {
    Node::new(NodeId::new(id), width, height)
}

fn edge(id: &str, from: &str, to: &str) -> Edge {
    Edge::new(EdgeId::new(id), NodeId::new(from), NodeId::new(to))
}

fn single() -> LayoutGraph {
    let mut g = LayoutGraph::new();
    g.add_node(node("only", 120.0, 60.0));
    g
}

fn chain(len: usize) -> LayoutGraph {
    let mut g = LayoutGraph::new();
    for index in 0..len {
        g.add_node(node(&format!("n{index}"), 120.0, 60.0));
    }
    for index in 1..len {
        g.add_edge(edge(
            &format!("r{index}"),
            &format!("n{}", index - 1),
            &format!("n{index}"),
        ));
    }
    g
}

fn diamond() -> LayoutGraph {
    let mut g = LayoutGraph::new();
    for id in ["top", "left", "right", "bottom"] {
        g.add_node(node(id, 140.0, 70.0));
    }
    g.add_edge(edge("tl", "top", "left"));
    g.add_edge(edge("tr", "top", "right"));
    g.add_edge(edge("bl", "left", "bottom"));
    g.add_edge(edge("br", "right", "bottom"));
    g
}

fn fan_out() -> LayoutGraph {
    let mut g = LayoutGraph::new();
    g.add_node(node("hub", 160.0, 80.0));
    for index in 0..9 {
        g.add_node(node(&format!("leaf{index}"), 90.0, 45.0));
        g.add_edge(edge(&format!("r{index}"), "hub", &format!("leaf{index}")));
    }
    g
}

fn fan_in() -> LayoutGraph {
    let mut g = LayoutGraph::new();
    g.add_node(node("sink", 160.0, 80.0));
    for index in 0..9 {
        g.add_node(node(&format!("src{index}"), 90.0, 45.0));
        g.add_edge(edge(&format!("r{index}"), &format!("src{index}"), "sink"));
    }
    g
}

fn cycle() -> LayoutGraph {
    let mut g = chain(6);
    g.add_edge(edge("back", "n5", "n1"));
    g.add_edge(edge("back2", "n4", "n0"));
    g
}

fn self_loops() -> LayoutGraph {
    let mut g = LayoutGraph::new();
    g.add_node(node("a", 120.0, 60.0));
    g.add_node(node("b", 120.0, 60.0));
    g.add_edge(edge("ab", "a", "b"));
    g.add_edge(edge("ba", "b", "a"));
    g.add_edge(edge("loop-a1", "a", "a"));
    g.add_edge(edge("loop-a2", "a", "a"));
    g.add_edge(edge("loop-b", "b", "b"));
    g
}

fn parallel() -> LayoutGraph {
    let mut g = LayoutGraph::new();
    for id in ["a", "b", "c"] {
        g.add_node(node(id, 130.0, 65.0));
    }
    // Three relationships one way, two the other: the multigraph case.
    for index in 0..3 {
        g.add_edge(edge(&format!("fwd{index}"), "a", "b"));
    }
    for index in 0..2 {
        g.add_edge(edge(&format!("rev{index}"), "b", "a"));
    }
    g.add_edge(edge("bc", "b", "c"));
    g.add_edge(edge("ac", "a", "c"));
    g
}

fn disconnected() -> LayoutGraph {
    let mut g = LayoutGraph::new();
    g.add_node(node("island", 100.0, 50.0));
    for (index, (from, to)) in [("x1", "x2"), ("x2", "x3"), ("y1", "y2")]
        .into_iter()
        .enumerate()
    {
        for id in [from, to] {
            if g.nodes.iter().all(|n| n.id.as_str() != id) {
                g.add_node(node(id, 110.0, 55.0));
            }
        }
        g.add_edge(edge(&format!("r{index}"), from, to));
    }
    g
}

fn mixed_sizes() -> LayoutGraph {
    let mut g = LayoutGraph::new();
    let sizes = [
        (0.0, 0.0),
        (1.0, 1.0),
        (40.0, 400.0),
        (400.0, 40.0),
        (1_000.0, 12.0),
        (12.0, 1_000.0),
    ];
    for (index, (width, height)) in sizes.iter().enumerate() {
        g.add_node(node(&format!("s{index}"), *width, *height));
    }
    for index in 1..sizes.len() {
        g.add_edge(edge(
            &format!("r{index}"),
            &format!("s{}", index - 1),
            &format!("s{index}"),
        ));
    }
    g
}

fn odd_ids() -> LayoutGraph {
    // Ids never enter the engine, so these shapes must behave like any
    // other — and the result must still carry them back unchanged.
    let ids = ["a b", "key\"quote", "ключ", "a\nb", "n0", "e0", "\\slash"];
    let mut g = LayoutGraph::new();
    for id in ids {
        g.add_node(node(id, 100.0, 50.0));
    }
    for index in 1..ids.len() {
        g.add_edge(edge(&format!("r {index}"), ids[index - 1], ids[index]));
    }
    g
}

fn c4_like() -> LayoutGraph {
    // The shape of a component view: a few entry points over services,
    // databases at the bottom, realistic box sizes.
    let mut g = LayoutGraph::new();
    let boxes: &[(&str, f64, f64)] = &[
        ("person:web-user", 240.0, 120.0),
        ("person:ops", 240.0, 120.0),
        ("system:portal", 300.0, 160.0),
        ("container:frontend", 260.0, 140.0),
        ("container:api", 260.0, 140.0),
        ("container:worker", 260.0, 140.0),
        ("container:auth", 260.0, 140.0),
        ("database:sessions", 220.0, 130.0),
        ("database:ledger", 220.0, 130.0),
    ];
    for (id, width, height) in boxes {
        g.add_node(node(id, *width, *height));
    }
    let links: &[(&str, &str)] = &[
        ("person:web-user", "system:portal"),
        ("person:ops", "system:portal"),
        ("system:portal", "container:frontend"),
        ("container:frontend", "container:api"),
        ("container:api", "container:auth"),
        ("container:api", "container:worker"),
        ("container:worker", "database:ledger"),
        ("container:api", "database:sessions"),
        ("container:auth", "database:sessions"),
        ("container:frontend", "container:auth"),
    ];
    for (index, (from, to)) in links.iter().enumerate() {
        g.add_edge(edge(&format!("rel{index}"), from, to));
    }
    g
}

/// A pseudo-random graph with `nodes` nodes and about `edges` edges, back
/// edges included (so cycles are guaranteed) and varied box sizes.
fn tangled(nodes: usize, edges: usize) -> LayoutGraph {
    let mut g = LayoutGraph::new();
    let mut rng = Lcg::new(0x5de1_7ea5_9b31_c0de);
    for index in 0..nodes {
        let width = 60.0 + f64::from(u32::try_from(rng.below(240)).unwrap_or(0));
        let height = 40.0 + f64::from(u32::try_from(rng.below(160)).unwrap_or(0));
        g.add_node(node(&format!("t{index}"), width, height));
    }
    for index in 0..edges {
        let source = rng.below(u64::try_from(nodes).unwrap_or(1));
        let target = rng.below(u64::try_from(nodes).unwrap_or(1));
        g.add_edge(edge(
            &format!("te{index}"),
            &format!("t{source}"),
            &format!("t{target}"),
        ));
    }
    g
}

/// A wide layered grid: `ranks` of `width` nodes each, every node linked to
/// a couple of nodes in the next rank (the shape a large deployment view
/// approximates).
fn layered_grid(ranks: usize, width: usize) -> LayoutGraph {
    let mut g = LayoutGraph::new();
    let mut rng = Lcg::new(0x0de1_7ea5_9b31_c0de);
    for rank in 0..ranks {
        for slot in 0..width {
            g.add_node(node(&format!("r{rank}s{slot}"), 100.0, 50.0));
        }
    }
    for rank in 1..ranks {
        for slot in 0..width {
            let links = 1 + rng.below(3);
            for _ in 0..links {
                let from = rng.below(u64::try_from(width).unwrap_or(1));
                g.add_edge(edge(
                    &format!("g{rank}-{slot}-{from}"),
                    &format!("r{}s{from}", rank - 1),
                    &format!("r{rank}s{slot}"),
                ));
            }
        }
    }
    g
}
