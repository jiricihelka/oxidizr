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
//!    emitted only when `generate_vertices`.
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

/// Barycenter rounds (§P3-D5 step 4): each round is one down + one
/// up sweep. Fixed count — determinism (§P3-D5) needs fixed passes.
const BARYCENTER_ROUNDS: usize = 4;

/// Priority-method passes (§P3-D5 step 5), alternating the neighbour
/// side (rank-1, then rank+1). Fixed count, ditto.
const PRIORITY_ROUNDS: usize = 4;

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
    /// Chain neighbours one rank down / up (self-loops connect
    /// nothing); consecutive chain vertices are rank-adjacent by
    /// construction, so these drive ordering and the priority passes.
    preds: Vec<Vec<usize>>,
    succs: Vec<Vec<usize>>,
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

        // Step 4: chain adjacency, then barycenter ordering (M2-3).
        let mut preds = vec![Vec::new(); verts.len()];
        let mut succs = vec![Vec::new(); verts.len()];
        for chain in &chains {
            if chain.verts[0] == chain.verts[1] {
                continue; // self-loop chain: no rank neighbours
            }
            for pair in chain.verts.windows(2) {
                succs[pair[0]].push(pair[1]);
                preds[pair[1]].push(pair[0]);
            }
        }
        Self::order(&mut ranks, &preds, &succs);
        Ok(Self {
            verts,
            ranks,
            chains,
            preds,
            succs,
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

    /// Step 4: barycenter sweeps (§P3-M2 M2-3). A fixed number of
    /// rounds, each a down sweep (every rank re-sorted by the mean
    /// position of its predecessors) and an up sweep (successors);
    /// vertices without such neighbours keep their position as the
    /// barycentre, and the stable sort makes ties fall to the current
    /// order. After each sweep the crossing count is taken and the
    /// best order kept (ties keep the EARLIER order, so the initial
    /// one beats a tied sweep result). Fixed rounds, `total_cmp` on
    /// finite means, stable sorts, integer crossings — deterministic
    /// (§P3-D5).
    fn order(ranks: &mut Vec<Vec<usize>>, preds: &[Vec<usize>], succs: &[Vec<usize>]) {
        let mut best = ranks.clone();
        let mut best_crossings = crossings(ranks, preds);
        for _ in 0..BARYCENTER_ROUNDS {
            Self::sweep(ranks, preds, true);
            let counted = crossings(ranks, preds);
            if counted < best_crossings {
                ranks.clone_into(&mut best);
                best_crossings = counted;
            }
            Self::sweep(ranks, succs, false);
            let counted = crossings(ranks, preds);
            if counted < best_crossings {
                ranks.clone_into(&mut best);
                best_crossings = counted;
            }
        }
        *ranks = best;
    }

    /// One barycenter sweep. `down` selects the swept ranks (skipping
    /// the fixed boundary side) and `side` the neighbour lists that
    /// hold the fixed positions. Positions reflect the CURRENT rank
    /// state, so each swept rank barycentres against already-swept
    /// neighbours — the standard cascading sweep.
    #[allow(clippy::cast_precision_loss)]
    // Positions and sums of positions: far below f64's exact range.
    fn sweep(ranks: &mut [Vec<usize>], side: &[Vec<usize>], down: bool) {
        let mut pos = vec![0_usize; side.len()];
        for ids in ranks.iter() {
            for (rank_pos, &vid) in ids.iter().enumerate() {
                pos[vid] = rank_pos;
            }
        }
        let swept: Vec<usize> = if down {
            (1..ranks.len()).collect()
        } else {
            (0..ranks.len().saturating_sub(1)).rev().collect()
        };
        for ri in swept {
            let ids = std::mem::take(&mut ranks[ri]);
            let mut keyed: Vec<(f64, usize)> = ids
                .iter()
                .enumerate()
                .map(|(rank_pos, &vid)| {
                    let nbs = &side[vid];
                    if nbs.is_empty() {
                        (pos[vid] as f64, rank_pos)
                    } else {
                        let sum: usize = nbs.iter().map(|&nbr| pos[nbr]).sum();
                        (sum as f64 / nbs.len() as f64, rank_pos)
                    }
                })
                .collect();
            keyed.sort_by(|one, two| one.0.total_cmp(&two.0).then(one.1.cmp(&two.1)));
            let sorted: Vec<usize> = keyed.iter().map(|key| ids[key.1]).collect();
            for (rank_pos, &vid) in sorted.iter().enumerate() {
                pos[vid] = rank_pos;
            }
            ranks[ri] = sorted;
        }
    }

    /// Step 5 refinement: the simple PRIORITY METHOD (§P3-M2 M2-3).
    /// Fixed alternating passes; each pass pulls every vertex toward
    /// the mean CENTRE of one rank side's neighbours (current centre
    /// when it has none on that side) and re-places each rank in
    /// priority order — dummies first, so long-edge chains straighten,
    /// reals after them by current order. The move is clamped to the
    /// slot the within-rank neighbours leave open (placed neighbours
    /// bound with their LIVE centre, unplaced ones with their pass-
    /// start centre), so rank order and `node_separation` are
    /// preserved by construction; an over-constrained vertex resolves
    /// to the upper bound (deterministic). The last pass wins; a
    /// final shift returns the lowest box to the zero line.
    #[allow(clippy::cast_precision_loss)]
    // Means of exact coordinate sums over small degree counts.
    fn priority(&self, verts: &mut [Vertex], node_sep: f64) {
        for round in 0..PRIORITY_ROUNDS {
            let side = if round % 2 == 0 {
                &self.preds
            } else {
                &self.succs
            };
            let start: Vec<f64> = verts.iter().map(|vert| vert.b + vert.cb / 2.0).collect();
            for ids in &self.ranks {
                let desired: Vec<f64> = ids
                    .iter()
                    .map(|&vid| {
                        let nbs = &side[vid];
                        if nbs.is_empty() {
                            start[vid]
                        } else {
                            nbs.iter().map(|&nbr| start[nbr]).sum::<f64>() / nbs.len() as f64
                        }
                    })
                    .collect();
                let mut order: Vec<(u8, usize)> = ids
                    .iter()
                    .enumerate()
                    .map(|(slot, &vid)| {
                        let prio = u8::from(verts[vid].is_dummy());
                        (prio, slot)
                    })
                    .collect();
                order.sort_by(|one, two| two.0.cmp(&one.0).then(one.1.cmp(&two.1)));
                let mut placed = vec![false; ids.len()];
                for &(_, slot) in &order {
                    let vid = ids[slot];
                    let half = verts[vid].cb / 2.0;
                    let mut low = f64::NEG_INFINITY;
                    let mut high = f64::INFINITY;
                    if slot > 0 {
                        let nbr = ids[slot - 1];
                        let nbr_center = if placed[slot - 1] {
                            verts[nbr].b + verts[nbr].cb / 2.0
                        } else {
                            start[nbr]
                        };
                        low = nbr_center + verts[nbr].cb / 2.0 + node_sep + half;
                    }
                    if slot + 1 < ids.len() {
                        let nbr = ids[slot + 1];
                        let nbr_center = if placed[slot + 1] {
                            verts[nbr].b + verts[nbr].cb / 2.0
                        } else {
                            start[nbr]
                        };
                        high = nbr_center - verts[nbr].cb / 2.0 - node_sep - half;
                    }
                    let mut target = desired[slot];
                    if target < low {
                        target = low;
                    }
                    if target > high {
                        target = high;
                    }
                    // Over-constrained (low > high): the clamp order
                    // above resolves to `high` — deterministic.
                    verts[vid].b = target - half;
                    placed[slot] = true;
                }
            }
        }
        let lowest = verts
            .iter()
            .filter(|vert| !vert.is_dummy())
            .map(|vert| vert.b)
            .fold(f64::INFINITY, f64::min);
        if lowest.is_finite() {
            for vert in &mut *verts {
                vert.b -= lowest;
            }
        }
    }

    /// Steps 5+6: stacked canonical coordinates refined by the
    /// priority method, mapped through the rank direction, plus
    /// orthogonalised anchor-anchored routes.
    #[allow(clippy::too_many_lines)]
    // One straight-line pass (rank advance, cross stacking, priority
    // passes, direction mapping, routes); extraction would thread
    // five intermediates.
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
        // Step 5 refinement: straighten chains / shorten edges, then
        // return the lowest box to the zero line (§P3-M2 M2-3).
        self.priority(&mut verts, node_sep);

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
                horizontal,
            );
            edges.push(EdgePlacement {
                id: edge.id.clone(),
                points,
            });
        }
        LayoutResult { nodes, edges }
    }
}

/// Edge crossings between consecutive ranks (§P3-M2 M2-3 ordering
/// metric): every pair of edges whose rank-`ri-1` endpoints order
/// inverts against their rank-`ri` endpoints counts once; edges
/// sharing an endpoint never cross. All predecessors sit exactly one
/// rank up by construction, so the rank-pair split is exact.
/// O(e²) per rank pair — sparse C4 graphs keep this cheap (§P3-D5
/// quality bar, not Graphviz-grade crossing minimisation).
fn crossings(ranks: &[Vec<usize>], preds: &[Vec<usize>]) -> usize {
    let mut pos = vec![0_usize; preds.len()];
    for ids in ranks {
        for (rank_pos, &vid) in ids.iter().enumerate() {
            pos[vid] = rank_pos;
        }
    }
    let mut total = 0_usize;
    for lower in ranks.iter().skip(1) {
        let mut between: Vec<(usize, usize)> = Vec::new();
        for &vid in lower {
            for &nbr in &preds[vid] {
                between.push((pos[nbr], pos[vid]));
            }
        }
        between.sort_unstable();
        for one in 0..between.len() {
            for two in one + 1..between.len() {
                if between[one].0 < between[two].0 && between[one].1 > between[two].1 {
                    total += 1;
                }
            }
        }
    }
    total
}

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

/// Step 6: anchor → dummy centres (only when `generate_vertices`) →
/// anchor, walked in the INPUT direction — chains are stored
/// acyclic-oriented, so a reversed edge's chain runs backwards here.
/// Diagonal legs are orthogonalised through channels at the mid rank
/// coordinate — a rank GAP, never a rank band — or at the mid cross
/// coordinate when both ends attach to the cross sides (§P3-D5
/// "orthogonal-ish" MVP; the polish half is P4-D13). Without
/// `generate_vertices` the route is the straight anchor→anchor pair.
#[allow(clippy::too_many_arguments)] // one structural bundle per argument
fn route(
    edge: &Edge,
    chain: &Chain,
    verts: &[Vertex],
    centers: &[Point],
    boxes: &[(f64, f64, f64, f64)],
    generate_vertices: bool,
    horizontal: bool,
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
    let from = endpoint(boxes[si], edge.from, toward, horizontal);
    let back = interior
        .last()
        .map_or_else(|| center(boxes[si]), |&vid| centers[vid]);
    let to = endpoint(boxes[ti], edge.to, back, horizontal);
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
// half-sums, §P3-D5 determinism): equality here is the defined test
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

/// The anchor point of `box_rect` with its side kind: the named
/// side's midpoint when pinned, otherwise the side facing `toward`
/// (dominant delta wins, exact ties take the horizontal sides).
fn endpoint(
    box_rect: (f64, f64, f64, f64),
    anchor: Option<Anchor>,
    toward: Point,
    horizontal: bool,
) -> (Point, SideKind) {
    let (bx, by, bw, bh) = box_rect;
    let (rank_side, cross_side) = if horizontal {
        (SideKind::Rank, SideKind::Cross)
    } else {
        (SideKind::Cross, SideKind::Rank)
    };
    match anchor {
        Some(Anchor::Top) => (
            Point {
                x: bx + bw / 2.0,
                y: by,
            },
            cross_side,
        ),
        Some(Anchor::Bottom) => (
            Point {
                x: bx + bw / 2.0,
                y: by + bh,
            },
            cross_side,
        ),
        Some(Anchor::Left) => (
            Point {
                x: bx,
                y: by + bh / 2.0,
            },
            rank_side,
        ),
        Some(Anchor::Right) => (
            Point {
                x: bx + bw,
                y: by + bh / 2.0,
            },
            rank_side,
        ),
        None => {
            let mid = center(box_rect);
            let dx = toward.x - mid.x;
            let dy = toward.y - mid.y;
            if dx.abs() >= dy.abs() {
                (
                    Point {
                        x: if dx >= 0.0 { bx + bw } else { bx },
                        y: by + bh / 2.0,
                    },
                    rank_side,
                )
            } else {
                (
                    Point {
                        x: bx + bw / 2.0,
                        y: if dy >= 0.0 { by + bh } else { by },
                    },
                    cross_side,
                )
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
        // x 200 / x 360 around the dummy's 110 line.
        assert_eq!(
            pts(&r, "ac"),
            vec![
                Point { x: 120.0, y: 70.0 },
                Point { x: 200.0, y: 70.0 },
                Point { x: 200.0, y: 110.0 },
                Point { x: 360.0, y: 110.0 },
                Point { x: 360.0, y: 70.0 },
                Point { x: 440.0, y: 70.0 },
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
        // The §P3-D5 orthogonal-ish promise: with vertices generated,
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

    #[test]
    fn thousand_nodes_are_deterministic() {
        // §P3-M2 M2-3 scale gate: the whole pipeline (sweeps, crossing
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
    }
}
