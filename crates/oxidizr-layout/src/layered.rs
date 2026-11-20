//! [`Layered`] — the Sugiyama-family layered engine, **P3 MVP scope**
//! (§P3-D5; the quality-polish half is P4-D13).
//!
//! Pipeline, in the order §P3-D5 locks it:
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
//! 4. **ordering** — MVP: the deterministic initial order (real nodes
//!    by node insertion index, dummies by edge insertion order);
//!    barycenter sweeps land with the §P3-M2 M2-3 row;
//! 5. **coordinates** — simple stacking: ranks advance by the rank
//!    axis' largest node + `rank_separation`, ranks stack along the
//!    cross axis with `node_separation`, centred on the shared
//!    mid-line; the priority pass is M2-3's;
//! 6. **edge routes** — polyline anchor → dummy centres → anchor,
//!    interior points emitted only when `generate_vertices`.
//!
//! `rank_direction` decides which world axis is the rank axis and
//! which way it runs: ranks always advance left→right / top→down in
//! the canonical frame, mirrored for `RightLeft` / `BottomUp`.
//!
//! **Determinism (hard contract, §P3-D5):** no randomness, stable
//! sorts only, fixed passes — the same input graph always produces an
//! identical [`LayoutResult`].

use std::collections::{HashMap, VecDeque};

use crate::engine::{EdgePlacement, LayoutEngine, LayoutError, LayoutResult, NodePlacement, Point};
use crate::graph::{Anchor, Edge, LayoutGraph, RankDirection};

/// The layered engine (§P3-D5). Stateless today; kept as a unit struct
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

/// One laid-out vertex: a real node or a rank-skipping dummy. Sizes
/// are CANONICAL: `ra` along the rank axis, `cb` along the cross axis
/// (swapped from the input `width`/`height` for vertical directions).
#[derive(Debug, Clone, Copy)]
struct Vertex {
    /// Index into `LayoutGraph::nodes`, or [`Vertex::DUMMY`].
    real: usize,
    rank: usize,
    /// Canonical box: rank-axis start, cross-axis start, extents.
    a: f64,
    b: f64,
    ra: f64,
    cb: f64,
}

impl Vertex {
    const DUMMY: usize = usize::MAX;

    fn is_dummy(self) -> bool {
        self.real == Self::DUMMY
    }
}

/// A forward edge between node indices, flagged when the DFS reversed
/// it (the input direction stays recoverable).
#[derive(Debug, Clone, Copy)]
struct Oriented {
    from: usize,
    to: usize,
    reversed: bool,
}

impl Oriented {
    /// Endpoints in the ACYCLIC direction (input order when kept).
    fn ends(self) -> (usize, usize) {
        if self.reversed {
            (self.to, self.from)
        } else {
            (self.from, self.to)
        }
    }
}

/// The acyclic, layered, dummy-expanded skeleton of one run
/// (§P3-D5 steps 1-4; coordinates and routes are [`Self::place`]).
struct Skeleton {
    verts: Vec<Vertex>,
    /// Vertex ids per rank, in layout order (deterministic).
    ranks: Vec<Vec<usize>>,
    /// One chain per input edge, in edge insertion order, oriented
    /// along the acyclic direction (`reversed` marks the edges where
    /// that differs from the input direction).
    chains: Vec<Chain>,
}

/// One edge's vertex chain: endpoints plus dummies across skipped
/// ranks (§P3-D5 step 3). Length is always ≥ 2.
struct Chain {
    reversed: bool,
    verts: Vec<usize>,
}

impl Skeleton {
    fn build(graph: &LayoutGraph) -> Result<Self, LayoutError> {
        let n = graph.nodes.len();
        let oriented = Self::acyclic(graph);

        // Step 2: longest-path layering over the acyclic orientation.
        let mut indeg = vec![0_usize; n];
        let mut succ = vec![Vec::new(); n];
        for edge in &oriented {
            let (s, t) = edge.ends();
            if s == t {
                continue; // self-loops take part in no rank
            }
            succ[s].push(t);
            indeg[t] += 1;
        }
        let mut rank = vec![0_usize; n];
        let mut queue: VecDeque<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
        let mut popped = 0_usize;
        while let Some(u) = queue.pop_front() {
            popped += 1;
            for &t in &succ[u] {
                rank[t] = rank[t].max(rank[u] + 1);
                indeg[t] -= 1;
                if indeg[t] == 0 {
                    queue.push_back(t);
                }
            }
        }
        if popped != n {
            // Defensive only: step 1 guarantees a DAG.
            return Err(LayoutError::CycleRemovalFailed);
        }

        // Steps 3+4: dummy vertices for multi-rank spans, then the
        // initial per-rank order (reals by node index, then dummies by
        // creation order — `verts` is already in exactly that order).
        let mut verts: Vec<Vertex> = graph
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| Vertex {
                real: i,
                rank: rank[i],
                a: 0.0,
                b: 0.0,
                ra: node.width,
                cb: node.height,
            })
            .collect();
        let mut chains = Vec::with_capacity(oriented.len());
        for edge in &oriented {
            let (s, t) = edge.ends();
            let mut chain = Chain {
                reversed: edge.reversed,
                verts: Vec::with_capacity(2),
            };
            if s == t {
                chain.verts.extend_from_slice(&[s, s]);
            } else {
                chain.verts.push(s);
                for r in rank[s] + 1..rank[t] {
                    verts.push(Vertex {
                        real: Vertex::DUMMY,
                        rank: r,
                        a: 0.0,
                        b: 0.0,
                        ra: 0.0,
                        cb: 0.0,
                    });
                    let dummy = verts.len() - 1;
                    chain.verts.push(dummy);
                }
                chain.verts.push(t);
            }
            chains.push(chain);
        }
        let ranks_len = rank.iter().copied().max().unwrap_or(0) + 1;
        let mut ranks = vec![Vec::new(); ranks_len];
        for (vid, v) in verts.iter().enumerate() {
            ranks[v.rank].push(vid);
        }
        Ok(Self {
            verts,
            ranks,
            chains,
        })
    }

    /// Step 1: iterative DFS marking every back edge as reversed.
    /// Roots and adjacency walk insertion order, so the acyclic
    /// orientation is stable for equal inputs.
    fn acyclic(graph: &LayoutGraph) -> Vec<Oriented> {
        const WHITE: u8 = 0;
        const GRAY: u8 = 1;
        const BLACK: u8 = 2;
        let n = graph.nodes.len();
        let mut index = HashMap::with_capacity(n);
        for (i, node) in graph.nodes.iter().enumerate() {
            index.insert(node.id.as_str(), i);
        }
        // `validate` has run, so both endpoints always resolve; the
        // fallback keeps lengths aligned without any panicking index.
        let ends: Vec<(usize, usize)> = graph
            .edges
            .iter()
            .map(|e| {
                (
                    index.get(e.source.as_str()).copied().unwrap_or(0),
                    index.get(e.target.as_str()).copied().unwrap_or(0),
                )
            })
            .collect();
        let mut out: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
        for (idx, &(s, t)) in ends.iter().enumerate() {
            if s != t {
                out[s].push((t, idx));
            }
        }

        let mut color = vec![WHITE; n];
        let mut reversed = vec![false; ends.len()];
        for root in 0..n {
            if color[root] != WHITE {
                continue;
            }
            color[root] = GRAY;
            let mut stack: Vec<(usize, usize)> = vec![(root, 0)];
            while let Some(top) = stack.last_mut() {
                let (u, next) = *top;
                if next < out[u].len() {
                    top.1 += 1;
                    let (v, e) = out[u][next];
                    match color[v] {
                        WHITE => {
                            color[v] = GRAY;
                            stack.push((v, 0));
                        }
                        GRAY => reversed[e] = true,
                        _ => {}
                    }
                } else {
                    color[u] = BLACK;
                    stack.pop();
                }
            }
        }
        ends.iter()
            .enumerate()
            .map(|(idx, &(from, to))| Oriented {
                from,
                to,
                reversed: reversed[idx],
            })
            .collect()
    }

    /// Steps 5+6: stacked canonical coordinates mapped through the
    /// rank direction, plus anchor-anchored routes.
    #[allow(clippy::too_many_lines)]
    // One straight-line pass (rank advance, cross stacking, direction
    // mapping, routes); extraction would thread five intermediates.
    fn place(&self, graph: &LayoutGraph) -> LayoutResult {
        let horizontal = matches!(
            graph.config.rank_direction,
            RankDirection::LeftRight | RankDirection::RightLeft
        );
        let rank_sep = f64::from(graph.config.rank_separation.max(0));
        let node_sep = f64::from(graph.config.node_separation.max(0));

        // Canonical extents: the rank axis carries HEIGHT in vertical
        // layouts and WIDTH otherwise (dummies are 0×0 either way).
        let mut verts = self.verts.clone();
        if !horizontal {
            for v in &mut verts {
                if !v.is_dummy() {
                    std::mem::swap(&mut v.ra, &mut v.cb);
                }
            }
        }

        let rank_max_a: Vec<f64> = self
            .ranks
            .iter()
            .map(|r| r.iter().map(|&v| verts[v].ra).fold(0.0_f64, f64::max))
            .collect();
        let mut rank_a = vec![0.0_f64; self.ranks.len()];
        for ri in 1..self.ranks.len() {
            rank_a[ri] = rank_a[ri - 1] + rank_max_a[ri - 1] + rank_sep;
        }

        // Cross-axis stacking, every rank centred on the mid-line.
        let mut content = vec![0.0_f64; self.ranks.len()];
        for (ri, ids) in self.ranks.iter().enumerate() {
            let mut cursor = 0.0_f64;
            for &vid in ids {
                verts[vid].b = cursor;
                cursor += verts[vid].cb + node_sep;
            }
            content[ri] = (cursor - node_sep).max(0.0);
        }
        let mid = content.iter().copied().fold(0.0_f64, f64::max) / 2.0;
        for (ri, ids) in self.ranks.iter().enumerate() {
            let shift = mid - content[ri] / 2.0;
            for &vid in ids {
                verts[vid].a = rank_a[ri] + (rank_max_a[ri] - verts[vid].ra) / 2.0;
                verts[vid].b += shift;
            }
        }

        let along = rank_a.last().copied().unwrap_or(0.0) + *rank_max_a.last().unwrap_or(&0.0);
        let dir = graph.config.rank_direction;
        // Canonical (rank-axis x, cross-axis y) → world box top-left.
        let map_box = |vert: &Vertex| match dir {
            RankDirection::LeftRight => Point {
                x: vert.a,
                y: vert.b,
            },
            RankDirection::RightLeft => Point {
                x: along - vert.a - vert.ra,
                y: vert.b,
            },
            RankDirection::TopDown => Point {
                x: vert.b,
                y: vert.a,
            },
            RankDirection::BottomUp => Point {
                x: vert.b,
                y: along - vert.a - vert.ra,
            },
        };
        // Canonical point → world point (a point has no extent to
        // subtract, so mirroring is the plain flip).
        let map_point = |pt: Point| match dir {
            RankDirection::LeftRight => pt,
            RankDirection::RightLeft => Point {
                x: along - pt.x,
                y: pt.y,
            },
            RankDirection::TopDown => Point { x: pt.y, y: pt.x },
            RankDirection::BottomUp => Point {
                x: pt.y,
                y: along - pt.x,
            },
        };

        // World-space boxes (input w/h — the canonical swap maps back)
        // and every vertex's world centre, for the route pass.
        let mut boxes = vec![(0.0, 0.0, 0.0, 0.0); graph.nodes.len()];
        let mut nodes = Vec::with_capacity(graph.nodes.len());
        for (i, node) in graph.nodes.iter().enumerate() {
            let world = map_box(&verts[i]);
            let (out_w, out_h) = if horizontal {
                (verts[i].ra, verts[i].cb)
            } else {
                (verts[i].cb, verts[i].ra)
            };
            boxes[i] = (world.x, world.y, out_w, out_h);
            nodes.push(NodePlacement {
                id: node.id.clone(),
                position: world,
            });
        }
        let centers: Vec<Point> = verts
            .iter()
            .map(|vert| {
                map_point(Point {
                    x: vert.a + vert.ra / 2.0,
                    y: vert.b + vert.cb / 2.0,
                })
            })
            .collect();

        let generate_vertices = graph.config.generate_vertices;
        let mut edges = Vec::with_capacity(graph.edges.len());
        for (idx, edge) in graph.edges.iter().enumerate() {
            let points = route(
                edge,
                &self.chains[idx],
                &verts,
                &centers,
                &boxes,
                generate_vertices,
            );
            edges.push(EdgePlacement {
                id: edge.id.clone(),
                points,
            });
        }
        LayoutResult { nodes, edges }
    }
}

/// Step 6: anchor → dummy centres (only when `generate_vertices`) →
/// anchor, walked in the INPUT direction — chains are stored
/// acyclic-oriented, so a reversed edge's chain runs backwards here.
#[allow(clippy::too_many_arguments)] // one structural bundle per argument
fn route(
    edge: &Edge,
    chain: &Chain,
    verts: &[Vertex],
    centers: &[Point],
    boxes: &[(f64, f64, f64, f64)],
    generate_vertices: bool,
) -> Vec<Point> {
    let last = chain.verts.len() - 1;
    let (start_vid, end_vid) = if chain.reversed {
        (chain.verts[last], chain.verts[0])
    } else {
        (chain.verts[0], chain.verts[last])
    };
    let (si, ti) = (verts[start_vid].real, verts[end_vid].real);
    if si == ti {
        // Self-loop placeholder route (P4-D13 polish): two stubs on
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
    let interior: Vec<Point> = if generate_vertices {
        let skipped = &chain.verts[1..last];
        if chain.reversed {
            skipped.iter().rev().map(|&v| centers[v]).collect()
        } else {
            skipped.iter().map(|&v| centers[v]).collect()
        }
    } else {
        Vec::new()
    };
    let toward = interior
        .first()
        .copied()
        .unwrap_or_else(|| center(boxes[ti]));
    let from = endpoint(boxes[si], edge.from, toward);
    let back = interior
        .last()
        .copied()
        .unwrap_or_else(|| center(boxes[si]));
    let to = endpoint(boxes[ti], edge.to, back);
    let mut points = Vec::with_capacity(interior.len() + 2);
    points.push(from);
    points.extend(interior);
    points.push(to);
    points
}

/// Centre of a world-space box.
fn center(box_rect: (f64, f64, f64, f64)) -> Point {
    let (x, y, w, h) = box_rect;
    Point {
        x: x + w / 2.0,
        y: y + h / 2.0,
    }
}

/// The anchor point of `box_rect`: the named side's midpoint when
/// pinned, otherwise the side facing `toward` (dominant delta wins,
/// exact ties take the horizontal sides).
fn endpoint(box_rect: (f64, f64, f64, f64), anchor: Option<Anchor>, toward: Point) -> Point {
    let (bx, by, bw, bh) = box_rect;
    match anchor {
        Some(Anchor::Top) => Point {
            x: bx + bw / 2.0,
            y: by,
        },
        Some(Anchor::Bottom) => Point {
            x: bx + bw / 2.0,
            y: by + bh,
        },
        Some(Anchor::Left) => Point {
            x: bx,
            y: by + bh / 2.0,
        },
        Some(Anchor::Right) => Point {
            x: bx + bw,
            y: by + bh / 2.0,
        },
        None => {
            let mid = center(box_rect);
            let dx = toward.x - mid.x;
            let dy = toward.y - mid.y;
            if dx.abs() >= dy.abs() {
                Point {
                    x: if dx >= 0.0 { bx + bw } else { bx },
                    y: by + bh / 2.0,
                }
            } else {
                Point {
                    x: bx + bw / 2.0,
                    y: if dy >= 0.0 { by + bh } else { by },
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::graph::{EdgeId, Node, NodeId};

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
        // being acyclically a → c across rank 1, with a dummy waypoint.
        assert_eq!(
            pts(&r, "ca"),
            vec![
                Point { x: 440.0, y: 55.0 },
                Point { x: 280.0, y: 110.0 },
                Point { x: 120.0, y: 55.0 },
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
        assert_eq!(
            pts(&r, "ac"),
            vec![
                Point { x: 120.0, y: 55.0 },
                Point { x: 280.0, y: 110.0 },
                Point { x: 440.0, y: 55.0 },
            ]
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
        assert_eq!(
            pts(&layout(&g), "ab"),
            vec![Point { x: 60.0, y: 0.0 }, Point { x: 280.0, y: 60.0 }]
        );
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
        // The hard determinism contract (P3-D5): a mid-size cyclic
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
}
