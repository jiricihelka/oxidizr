//! The skeleton half of the layered engine: the vertex/dummy models, the
//! build pipeline (cycle removal, layering, dummy chains) and the ordering
//! passes — the barycenter sweeps with the crossings metric. The routing
//! legs live in `route`; the engine entry and the shared constants stay in
//! the parent module.

use std::collections::{HashMap, VecDeque};

use crate::engine::{EdgePlacement, LayoutError, LayoutResult, NodePlacement, Point};
use crate::graph::{LayoutGraph, RankDirection};

use super::{BARYCENTER_ROUNDS, PRIORITY_ROUNDS, port_spread, route, spread_channels};

/// One laid-out vertex: a real node or a rank-skipping dummy. Sizes
/// are CANONICAL: `ra` along the rank axis, `cb` along the cross axis
/// (swapped from the input `width`/`height` for vertical directions).
#[derive(Debug, Clone, Copy)]
pub(super) struct Vertex {
    /// Index into `LayoutGraph::nodes`, or [`Vertex::DUMMY`].
    pub(super) real: usize,
    pub(super) rank: usize,
    /// Canonical box: rank-axis start, cross-axis start, extents.
    pub(super) a: f64,
    pub(super) b: f64,
    pub(super) ra: f64,
    pub(super) cb: f64,
}

impl Vertex {
    pub(super) const DUMMY: usize = usize::MAX;

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
/// (pipeline steps 1-4; coordinates and routes are [`Self::place`]).
pub(super) struct Skeleton {
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
/// ranks (pipeline step 3). Length is always ≥ 2.
pub(super) struct Chain {
    pub(super) reversed: bool,
    pub(super) verts: Vec<usize>,
}

impl Skeleton {
    pub(super) fn build(graph: &LayoutGraph) -> Result<Self, LayoutError> {
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

        // Step 4: chain adjacency, then barycenter ordering.
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

    /// Step 4: barycenter sweeps. A fixed number of
    /// rounds, each a down sweep (every rank re-sorted by the mean
    /// position of its predecessors) and an up sweep (successors);
    /// vertices without such neighbours keep their position as the
    /// barycentre, and the stable sort makes ties fall to the current
    /// order. After each sweep the crossing count is taken and the
    /// best order kept (ties keep the EARLIER order, so the initial
    /// one beats a tied sweep result). Fixed rounds, `total_cmp` on
    /// finite means, stable sorts, integer crossings — deterministic.
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

    /// Step 5 refinement: the simple PRIORITY METHOD.
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
    pub(super) fn place(&self, graph: &LayoutGraph) -> LayoutResult {
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
        // return the lowest box to the zero line.
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
        // Port spread: edges that land on the SAME
        // (node, side) fan out along it by the far end's cross order,
        // so arrows stop stacking on one midpoint. A lone edge keeps ½.
        let alongs = port_spread(
            &self.chains,
            &graph.edges,
            &verts,
            &centers,
            &boxes,
            generate_vertices,
        );
        let mut routes: Vec<Vec<Point>> = Vec::with_capacity(graph.edges.len());
        for (idx, edge) in graph.edges.iter().enumerate() {
            routes.push(route(
                edge,
                &self.chains[idx],
                &verts,
                &centers,
                &boxes,
                generate_vertices,
                horizontal,
                alongs[idx],
            ));
        }
        // Pull overlapping channel legs onto distinct lanes.
        spread_channels(&mut routes, horizontal, rank_sep, node_sep);
        let edges = routes
            .into_iter()
            .zip(graph.edges.iter())
            .map(|(points, edge)| EdgePlacement {
                id: edge.id.clone(),
                points,
            })
            .collect();
        LayoutResult { nodes, edges }
    }
}

/// Edge crossings between consecutive ranks (the ordering metric):
/// every pair of edges whose rank-`ri-1` endpoints order
/// inverts against their rank-`ri` endpoints counts once; edges
/// sharing an endpoint never cross. All predecessors sit exactly one
/// rank up by construction, so the rank-pair split is exact.
/// O(e²) per rank pair — sparse C4 graphs keep this cheap (a quality
/// bar, not Graphviz-grade crossing minimisation).
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
