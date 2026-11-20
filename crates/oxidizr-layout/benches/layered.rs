//! §P3-M2 M2-4 mandated benches: the `Layered` pipeline at §P3-D5
//! scale targets (1k nodes). Three shapes exercise the hot phases
//! separately: `mixed` (crossing counts + priority passes over many
//! small ranks, dummies from the long spans and back edges),
//! `chain` (the longest rank axis — 1k ranks, trivial ordering),
//! `star` (one rank pair concentrating ~1k edges — the O(e²)
//! crossing count worst case). Setup is outside the timed loop; the
//! benches time the whole pure `layout` call (the §P3-D5 contract
//! surface). Baseline: saved as `P3` at M2 close (xtask bench).

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use oxidizr_layout::{Edge, EdgeId, Layered, LayoutEngine, LayoutGraph, Node, NodeId};

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
    let mut id = 0_u32;
    for i in 0..count {
        let target = (i + 1 + i % 7) % count;
        graph.add_edge(edge(id, i, target));
        id += 1;
        if i % 11 == 0 {
            let back = i.saturating_sub(23);
            graph.add_edge(edge(id, i, back));
            id += 1;
        }
    }
    graph
}

/// 1000-rank chain: the layering, sweeps and priority passes each
/// walk one vertex per rank; routes are all straight.
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
/// crossing count quadratic in one rank pair, ordering under maximal
/// fan-out.
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
            let result = Layered::new().layout(black_box(graph)).expect("valid");
            black_box((
                result.nodes.len(),
                result.edges.iter().map(|e| e.points.len()).sum::<usize>(),
            ));
        });
    });
}

fn layered_mixed(c: &mut Criterion) {
    bench_shape(c, "p3_layered_thousand_mixed", &mixed_graph());
}

fn layered_chain(c: &mut Criterion) {
    bench_shape(c, "p3_layered_thousand_chain", &chain_graph());
}

fn layered_star(c: &mut Criterion) {
    bench_shape(c, "p3_layered_thousand_star", &star_graph());
}

criterion_group!(benches, layered_mixed, layered_chain, layered_star);
criterion_main!(benches);
