//! P7-M1 benches: the `Dagre` pipeline on the SAME three 1k shapes as
//! `benches/layered.rs` (`mixed`, `chain`, `star` — see that header for what
//! each exercises), so `cargo xtask bench` records the two engines side by
//! side on identical inputs (risk R-5: dagre may be slower — the numbers get
//! recorded, not hidden). Same contract as the Layered bench: setup outside
//! the timed loop, the pure `layout` call timed.

use criterion::{Criterion, black_box};
use oxidizr_layout::{Dagre, Edge, EdgeId, LayoutEngine, LayoutGraph, Node, NodeId};

fn node(id: u32) -> Node {
    Node::new(NodeId::new(format!("n{id}")), 120.0, 60.0)
}

fn edge(id: u32, from: u32, to: u32) -> Edge {
    Edge::new(
        EdgeId::new(format!("e{id}")),
        NodeId::new(format!("n{from}")),
        NodeId::new(format!("n{to}")),
    )
}

/// 1000 nodes, one forward edge each with a stride pattern plus a
/// back edge every 11th: cycles, long spans, dummies — the
/// everything-happens shape.
fn mixed_graph() -> LayoutGraph {
    let count = 1000_u32;
    let mut graph = LayoutGraph::new();
    for i in 0..count {
        graph.add_node(node(i));
    }
    for i in 0..count - 1 {
        graph.add_edge(edge(i, i, i + 1));
    }
    for i in (0..count - 17).step_by(1) {
        graph.add_edge(edge(1000 + i, i, i + 17));
    }
    for i in (11..count).step_by(11) {
        graph.add_edge(edge(2000 + i, i, i - 11));
    }
    graph
}

/// 1000 nodes in a single chain: 1000 ranks, one vertex per rank;
/// routes are all straight.
fn chain_graph() -> LayoutGraph {
    let count = 1000_u32;
    let mut graph = LayoutGraph::new();
    for i in 0..count {
        graph.add_node(node(i));
    }
    for i in 0..count - 1 {
        graph.add_edge(edge(i, i, i + 1));
    }
    graph
}

/// One hub, 999 leaves: a single rank pair with 999 edges — the
/// ordering under maximal fan-out.
fn star_graph() -> LayoutGraph {
    let count = 1000_u32;
    let mut graph = LayoutGraph::new();
    for i in 0..count {
        graph.add_node(node(i));
    }
    for leaf in 1..count {
        graph.add_edge(edge(leaf, 0, leaf));
    }
    graph
}

fn bench_shape(c: &mut Criterion, name: &str, graph: &LayoutGraph) {
    c.bench_function(name, |b| {
        b.iter(|| {
            let result = Dagre::new().layout(black_box(graph)).expect("valid");
            black_box((
                result.nodes.len(),
                result.edges.iter().map(|e| e.points.len()).sum::<usize>(),
            ));
        });
    });
}

fn dagre_mixed(c: &mut Criterion) {
    bench_shape(c, "p7_dagre_thousand_mixed", &mixed_graph());
}

fn dagre_chain(c: &mut Criterion) {
    bench_shape(c, "p7_dagre_thousand_chain", &chain_graph());
}

fn dagre_star(c: &mut Criterion) {
    bench_shape(c, "p7_dagre_thousand_star", &star_graph());
}

fn main() {
    // Measured 2026-11-28: `mixed` runs ~5.7 s per layout, so the criterion
    // default (100 samples, one iteration minimum per sample) would cost
    // ~10 min for that one bench. 20 samples on these 1k shapes are stable
    // (spread < 2 %); the Layered bench keeps the default sample size — its
    // runs are two orders of magnitude faster (see the recorded numbers).
    // `configure_from_args` still lets an ad-hoc run override everything.
    let mut criterion = Criterion::default().sample_size(20).configure_from_args();
    dagre_mixed(&mut criterion);
    dagre_chain(&mut criterion);
    dagre_star(&mut criterion);
}
