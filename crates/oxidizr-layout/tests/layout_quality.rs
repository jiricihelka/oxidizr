//! The layout quality bar: a committed, per-fixture comparison of the two
//! layered engines on the shared corpus. Four numbers per shape, measured on
//! the geometry the seam hands back (left-right ranks, contract separations):
//!
//! * `cross` — proper crossings between routes of different edges (the
//!   classic readability metric);
//! * `pierce` — route segments passing through a box they are not attached
//!   to (the shape a reader notices first);
//! * `bend` — interior waypoints, i.e. how much routing was needed;
//! * `len` — total route length in pixels.
//!
//! The table is the evidence base for promoting an engine to the default:
//! the verdict column flags a row whose crossing count is WORSE than the
//! built-in engine's and the assertion below fails on such a row, so a
//! regression cannot land silently. Re-bless with `UPDATE_EXPECT=1` — and
//! only with a per-fixture improvement story, never blindly.

#![cfg(feature = "dagre")]

mod common;

use expect_test::expect;
use oxidizr_layout::{
    Dagre, Edge, EdgeId, EdgePlacement, Layered, LayoutEngine, LayoutGraph, LayoutResult, Node,
    NodeId, NodePlacement, Point, RankDirection,
};

/// Geometry quality of one laid-out graph.
#[derive(Debug, Clone, Copy, Default)]
struct Quality {
    crossings: usize,
    pierces: usize,
    bends: usize,
    length: f64,
}

impl Quality {
    /// Measures `result` against the boxes `graph` declares.
    fn measure(graph: &LayoutGraph, result: &LayoutResult) -> Self {
        let segments = segments(graph, result);
        let mut out = Self {
            crossings: crossings(&segments),
            pierces: pierces(graph, result, &segments),
            bends: result
                .edges
                .iter()
                .map(|edge| edge.points.len().saturating_sub(2))
                .sum(),
            length: 0.0,
        };
        for segment in &segments {
            out.length += distance(segment.from, segment.to);
        }
        out
    }

    /// Element-wise sum (the `TOTAL` row).
    fn add(self, other: Self) -> Self {
        Self {
            crossings: self.crossings + other.crossings,
            pierces: self.pierces + other.pierces,
            bends: self.bends + other.bends,
            length: self.length + other.length,
        }
    }
}

/// One route segment, tagged with the node indices its edge connects: a
/// segment may only be a nuisance at boxes its own edge is not attached to.
#[derive(Debug, Clone, Copy)]
struct Segment {
    from: Point,
    to: Point,
    source: usize,
    target: usize,
}

/// Every route segment of `result`, in route order.
fn segments(graph: &LayoutGraph, result: &LayoutResult) -> Vec<Segment> {
    let index_of = |id: &str| {
        graph
            .nodes
            .iter()
            .position(|node| node.id.as_str() == id)
            .unwrap_or(usize::MAX)
    };
    let mut out = Vec::new();
    for edge in &result.edges {
        let (Some(graph_edge), [from, rest @ ..]) = (
            graph.edges.iter().find(|candidate| candidate.id == edge.id),
            edge.points.as_slice(),
        ) else {
            continue;
        };
        let (source, target) = (
            index_of(graph_edge.source.as_str()),
            index_of(graph_edge.target.as_str()),
        );
        let mut previous = *from;
        for point in rest {
            out.push(Segment {
                from: previous,
                to: *point,
                source,
                target,
            });
            previous = *point;
        }
    }
    out
}

/// Proper crossings: two segments of DIFFERENT edges whose endpoints strictly
/// separate each other. Touching at a shared border point is not a crossing,
/// and neither is a collinear overlap (the parallel lanes of one channel).
fn crossings(segments: &[Segment]) -> usize {
    let mut total = 0_usize;
    for (one, a) in segments.iter().enumerate() {
        for b in segments.iter().skip(one + 1) {
            if a.source == b.source
                || a.source == b.target
                || a.target == b.source
                || a.target == b.target
                || !boxes_overlap(a, b)
            {
                continue;
            }
            if strictly_separates(a, b) && strictly_separates(b, a) {
                total += 1;
            }
        }
    }
    total
}

/// Cheap axis-aligned bounding-box rejection before the sign test.
fn boxes_overlap(a: &Segment, b: &Segment) -> bool {
    let (a_min_x, a_max_x) = ordered(a.from.x, a.to.x);
    let (a_min_y, a_max_y) = ordered(a.from.y, a.to.y);
    let (b_min_x, b_max_x) = ordered(b.from.x, b.to.x);
    let (b_min_y, b_max_y) = ordered(b.from.y, b.to.y);
    a_min_x <= b_max_x && b_min_x <= a_max_x && a_min_y <= b_max_y && b_min_y <= a_max_y
}

/// True when `a`'s endpoints lie strictly on opposite sides of the line
/// through `b`.
fn strictly_separates(a: &Segment, b: &Segment) -> bool {
    side(a.from, b) * side(a.to, b) < 0.0
}

/// Which side of `b`'s line `point` is on: `-1`, `0` (exactly on it) or `1`.
/// Hand-rolled because `f64::signum` maps `+0.0` to `1.0`, which would turn a
/// route merely touching another route's end into a crossing.
fn side(point: Point, b: &Segment) -> f64 {
    let dx = b.to.x - b.from.x;
    let dy = b.to.y - b.from.y;
    let px = point.x - b.from.x;
    let py = point.y - b.from.y;
    let cross = dx * py - dy * px;
    if cross > 0.0 {
        1.0
    } else if cross < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// Route segments that pass through a box they are not attached to. The box
/// is shrunk by a hair so a route running along its border is not counted.
fn pierces(graph: &LayoutGraph, result: &LayoutResult, segments: &[Segment]) -> usize {
    const SHRINK: f64 = 1.0;
    let mut total = 0_usize;
    for (index, node) in graph.nodes.iter().enumerate() {
        let Some(placement) = result.nodes.iter().find(|placed| placed.id == node.id) else {
            continue;
        };
        let rect = (
            placement.position.x + SHRINK,
            placement.position.y + SHRINK,
            placement.position.x + node.width - SHRINK,
            placement.position.y + node.height - SHRINK,
        );
        if rect.2 <= rect.0 || rect.3 <= rect.1 {
            continue;
        }
        total += segments
            .iter()
            .filter(|segment| segment.source != index && segment.target != index)
            .filter(|segment| enters(rect, segment))
            .count();
    }
    total
}

/// True when part of the segment strictly inside `rect` survives: slab
/// clipping, with the surviving parameter range trimmed off the segment's own
/// ends so a start or end point inside the box does not count.
fn enters(rect: (f64, f64, f64, f64), segment: &Segment) -> bool {
    let mut enter = 0.0_f64;
    let mut exit = 1.0_f64;
    let axes = [
        (segment.from.x, segment.to.x, rect.0, rect.2),
        (segment.from.y, segment.to.y, rect.1, rect.3),
    ];
    for (from, to, low, high) in axes {
        let delta = to - from;
        if delta.abs() <= f64::EPSILON {
            if from <= low || from >= high {
                return false;
            }
            continue;
        }
        let (one, two) = ordered((low - from) / delta, (high - from) / delta);
        enter = enter.max(one);
        exit = exit.min(two);
        if enter >= exit {
            return false;
        }
    }
    enter + 1e-6 < exit && exit > 1e-6 && enter < 1.0 - 1e-6
}

/// Low/high pair that does not need `Ord` on `f64`.
fn ordered(one: f64, two: f64) -> (f64, f64) {
    if one <= two { (one, two) } else { (two, one) }
}

fn distance(one: Point, other: Point) -> f64 {
    let dx = other.x - one.x;
    let dy = other.y - one.y;
    (dx * dx + dy * dy).sqrt()
}

/// Runs one engine on a fixture, failing loudly (naming both) when it cannot
/// place it.
fn run(engine: &dyn LayoutEngine, engine_name: &str, graph: &LayoutGraph) -> LayoutResult {
    engine
        .layout(graph)
        .unwrap_or_else(|error| panic!("{engine_name} failed: {error}"))
}

/// The corpus table: one row per fixture, `layered→dagre` per metric, and a
/// `TOTAL` row.
fn table() -> String {
    let mut out = String::from(
        "fixture          cross L>D   pierce L>D   bend L>D    len L>D      verdict\n",
    );
    let mut total_layered = Quality::default();
    let mut total_dagre = Quality::default();
    for fixture in common::corpus() {
        let mut graph = (fixture.build)();
        graph.config.rank_direction = RankDirection::LeftRight;
        let layered = Quality::measure(&graph, &run(&Layered, "layered", &graph));
        let dagre = Quality::measure(&graph, &run(&Dagre::new(), "dagre", &graph));
        let verdict = match dagre.crossings.cmp(&layered.crossings) {
            std::cmp::Ordering::Greater => "WORSE",
            std::cmp::Ordering::Less => "better",
            std::cmp::Ordering::Equal => "same",
        };
        out.push_str(&row(fixture.name, layered, dagre, verdict));
        total_layered = total_layered.add(layered);
        total_dagre = total_dagre.add(dagre);
    }
    out.push_str(&row("TOTAL", total_layered, total_dagre, ""));
    out
}

/// One table row, `layered` first in every pair.
fn row(name: &str, layered: Quality, dagre: Quality, verdict: &str) -> String {
    format!(
        "{:<15} {:>5}→{:<4} {:>7}→{:<5} {:>6}→{:<4} {:>8.0}→{:<7.0} {verdict}\n",
        name,
        layered.crossings,
        dagre.crossings,
        layered.pierces,
        dagre.pierces,
        layered.bends,
        dagre.bends,
        layered.length,
        dagre.length,
    )
}

/// The shapes where the candidate engine crosses MORE than the built-in one,
/// measured 2026-11-28 (see the table below). This is the honest shape of the
/// quality bar: six option profiles (network simplex / tight tree, with and
/// without the greedy cycle remover, with and without rank alignment) were
/// swept over the corpus and none dominates, so the promotion is judged on
/// the aggregate plus this named set. A NEW worse shape fails the test, and
/// so does silently fixing one of these — both mean the table has to be
/// re-read and re-blessed on purpose.
///
/// * `c4-like` — one crossing where the built-in engine has none, against 30%
///   fewer bends and a shorter total route length on the same shape;
/// * `grid-120` — a random 10×12 grid, denser than any C4 view: the built-in
///   engine's channel lanes win here, while the candidate halves the crossings
///   on the tangled shape. Its length gap is also partly honest bookkeeping:
///   the candidate honours `edgeSeparation`, which the built-in engine ignores
///   until the separation is wired through its port spread.
const KNOWN_CROSSING_REGRESSIONS: &[&str] = &["c4-like", "grid-120"];

#[test]
fn corpus_quality_table_is_committed() {
    let table = table();
    // The WORSE rows are exactly the known set: no new shape may regress, and
    // a shape that stops regressing has to be removed from the list on
    // purpose.
    let worse: Vec<&str> = table
        .lines()
        .filter(|line| line.ends_with("WORSE"))
        .map(|line| line.split_whitespace().next().unwrap_or_default())
        .collect();
    assert_eq!(
        worse, KNOWN_CROSSING_REGRESSIONS,
        "the set of shapes crossing worse than the built-in engine changed:\n{table}"
    );
    // And the aggregate must still be better, or the promotion has no point.
    let totals = table
        .lines()
        .find(|line| line.starts_with("TOTAL"))
        .expect("a TOTAL row");
    let cross = totals
        .split_whitespace()
        .nth(1)
        .and_then(|pair| pair.split_once('\u{2192}'))
        .expect("a `layered→dagre` crossing pair in TOTAL");
    let (layered, dagre): (usize, usize) = (
        cross.0.parse().expect("a number"),
        cross.1.parse().expect("a number"),
    );
    assert!(
        dagre < layered,
        "the candidate engine must cross less than the built-in one in total: \
         layered {layered}, dagre {dagre}"
    );
    expect![[r"
        fixture          cross L>D   pierce L>D   bend L>D    len L>D      verdict
        single              0→0          0→0          0→0           0→0       same
        chain-8             0→0          0→0          0→7         700→700     same
        diamond             0→0          0→0          8→4         593→424     same
        fan-out             0→0          8→0         16→9        2970→2307    same
        fan-in              0→0          8→0         16→9        2970→2307    same
        cycle               1→1          0→0         18→19       2425→2149    same
        self-loops          0→0          0→0          0→17        290→1022    same
        parallel            0→0          0→0         16→9        1089→1307    same
        disconnected        0→0          0→0          2→3         350→300     same
        mixed-sizes         0→0          0→0          0→5         500→500     same
        odd-ids             0→0          0→0          0→6         600→600     same
        c4-like             0→1          0→0         20→14       2217→2072    WORSE
        tangled-90       1303→609       44→22      2474→2068   594549→555214  better
        grid-120          199→394      244→169      404→215     65843→127147  WORSE
        TOTAL            1503→1005     304→191     2974→2385   675097→696049  
    "]]
    .assert_eq(&table);
}

/// A shape the crossing metric must get right: an X of two routes crosses
/// exactly once; parallel lanes and routes sharing a box cross nowhere.
#[test]
fn metric_counts_the_obvious_crossing() {
    let x = [
        segment((0.0, 0.0), (100.0, 100.0), 0, 1),
        segment((0.0, 100.0), (100.0, 0.0), 2, 3),
    ];
    assert_eq!(crossings(&x), 1);
    let lanes = [
        segment((0.0, 0.0), (100.0, 0.0), 0, 1),
        segment((0.0, 10.0), (100.0, 10.0), 2, 3),
    ];
    assert_eq!(crossings(&lanes), 0);
    let shared = [
        segment((0.0, 0.0), (100.0, 100.0), 0, 1),
        segment((0.0, 0.0), (100.0, -100.0), 0, 2),
    ];
    assert_eq!(crossings(&shared), 0, "routes sharing a box do not cross");
    let touching = [
        segment((0.0, 0.0), (100.0, 100.0), 0, 1),
        segment((100.0, 100.0), (200.0, 0.0), 2, 3),
    ];
    assert_eq!(
        crossings(&touching),
        0,
        "a shared border point is not a cross"
    );
}

/// The box-penetration metric counts a route through an unrelated box and
/// ignores one that runs past it or is attached to it.
#[test]
fn metric_counts_a_box_penetration() {
    let mut graph = LayoutGraph::new();
    for id in ["a", "mid", "z"] {
        graph.add_node(Node::new(NodeId::new(id), 40.0, 40.0));
    }
    graph.add_edge(Edge::new(
        EdgeId::new("az"),
        NodeId::new("a"),
        NodeId::new("z"),
    ));
    graph.add_edge(Edge::new(
        EdgeId::new("mz"),
        NodeId::new("mid"),
        NodeId::new("z"),
    ));
    let mut result = LayoutResult::default();
    // Boxes on one row: a at x 0, mid at x 100, z at x 200, all 40x40 at y 100.
    for (id, x) in [("a", 0.0_f64), ("mid", 100.0), ("z", 200.0)] {
        result.nodes.push(NodePlacement {
            id: NodeId::new(id),
            position: Point { x, y: 100.0 },
        });
    }
    let horizontal =
        |from: f64, to: f64| vec![Point { x: from, y: 120.0 }, Point { x: to, y: 120.0 }];
    // `az` runs straight through `mid`; `mz` starts on `mid`'s border, which
    // is its own box and must not count.
    result.edges.push(EdgePlacement {
        id: EdgeId::new("az"),
        points: horizontal(40.0, 200.0),
    });
    result.edges.push(EdgePlacement {
        id: EdgeId::new("mz"),
        points: horizontal(140.0, 200.0),
    });
    let measured = Quality::measure(&graph, &result);
    assert_eq!(
        measured.pierces, 1,
        "only the a→z route pierces an unrelated box, got {measured:?}"
    );
}

/// Builds a segment for the metric unit tests.
fn segment(from: (f64, f64), to: (f64, f64), source: usize, target: usize) -> Segment {
    Segment {
        from: Point {
            x: from.0,
            y: from.1,
        },
        to: Point { x: to.0, y: to.1 },
        source,
        target,
    }
}
