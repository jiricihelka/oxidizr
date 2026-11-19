//! Neutral graph input types ([`LayoutGraph`] and friends) — P3-D5 surface.
//!
//! Ids are opaque [`String`] newtypes: the viewer maps `oxidizr-core` ids in
//! and back (it keeps the side table), so this crate never learns what an
//! element or relationship is.

use std::fmt;

/// Caller-assigned node identity (opaque here).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(String);

impl NodeId {
    /// Wrap an id.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The id text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Caller-assigned edge identity (opaque here).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EdgeId(String);

impl EdgeId {
    /// Wrap an id.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The id text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EdgeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A box side an edge may be pinned to at its endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Top,
    Right,
    Bottom,
    Left,
}

/// A node with its measured size (the engine never measures text).
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Identity.
    pub id: NodeId,
    /// Measured width, px.
    pub width: f64,
    /// Measured height, px.
    pub height: f64,
}

impl Node {
    /// Node with the given measured size.
    #[must_use]
    pub fn new(id: NodeId, width: f64, height: f64) -> Self {
        Self { id, width, height }
    }
}

/// An edge between two nodes, optionally anchored at each end.
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    /// Identity.
    pub id: EdgeId,
    /// Source node.
    pub source: NodeId,
    /// Target node.
    pub target: NodeId,
    /// Source-side anchor (`None` = engine picks).
    pub from: Option<Anchor>,
    /// Target-side anchor (`None` = engine picks).
    pub to: Option<Anchor>,
}

impl Edge {
    /// Unanchored edge between two node ids.
    #[must_use]
    pub fn new(id: EdgeId, source: NodeId, target: NodeId) -> Self {
        Self {
            id,
            source,
            target,
            from: None,
            to: None,
        }
    }
}

/// Rank direction — the four `automaticLayout` directions (the viewer maps
/// `oxidizr-core`'s enum; names mirror it so the mapping is trivial).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RankDirection {
    TopDown,
    BottomUp,
    /// Reference `AutomaticLayout` default (verified from the reference
    /// constants — see `oxidizr-core` `AutomaticLayout::default`).
    #[default]
    LeftRight,
    RightLeft,
}

/// Engine configuration, mirroring the reference auto-layout dialog
/// (`auto-layout.jspf`) parameters. Separations match the reference
/// `AutomaticLayout` defaults (rank 100, node 50, edge 50, vertices true).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutConfig {
    /// Rank direction.
    pub rank_direction: RankDirection,
    /// Gap between ranks.
    pub rank_separation: i32,
    /// Gap between nodes within a rank.
    pub node_separation: i32,
    /// Gap between edges leaving/entering a node.
    pub edge_separation: i32,
    /// Generate edge vertices (waypoints); `false` ⇒ straight polylines.
    pub generate_vertices: bool,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            rank_direction: RankDirection::default(),
            rank_separation: 100,
            node_separation: 50,
            edge_separation: 50,
            generate_vertices: true,
        }
    }
}

/// One layout input: nodes, edges and config, in caller (deterministic)
/// order. Build with [`LayoutGraph::new`] + [`add_node`](Self::add_node) +
/// [`add_edge`](Self::add_edge); engines call [`validate`](Self::validate)
/// themselves.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayoutGraph {
    /// Nodes in insertion order.
    pub nodes: Vec<Node>,
    /// Edges in insertion order.
    pub edges: Vec<Edge>,
    /// Engine configuration.
    pub config: LayoutConfig,
}

impl LayoutGraph {
    /// Empty graph with the reference-default [`LayoutConfig`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a node.
    pub fn add_node(&mut self, node: Node) {
        self.nodes.push(node);
    }

    /// Append an edge.
    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    /// The structural contract every engine relies on: unique node ids, every
    /// edge endpoint resolvable, finite non-negative sizes, non-negative
    /// separations. The empty graph is valid (engines answer empty results).
    ///
    /// # Errors
    ///
    /// The [`LayoutError`](crate::LayoutError) naming the first violated
    /// invariant.
    pub fn validate(&self) -> Result<(), crate::LayoutError> {
        use crate::LayoutError;

        let mut seen = std::collections::HashSet::new();
        for node in &self.nodes {
            if !seen.insert(node.id.as_str()) {
                return Err(LayoutError::DuplicateNodeId(node.id.clone()));
            }
            if !(node.width.is_finite() && node.height.is_finite())
                || node.width < 0.0
                || node.height < 0.0
            {
                return Err(LayoutError::BadNodeSize {
                    id: node.id.clone(),
                    width: node.width,
                    height: node.height,
                });
            }
        }
        for edge in &self.edges {
            for end in [&edge.source, &edge.target] {
                if !seen.contains(end.as_str()) {
                    return Err(LayoutError::UnknownEdgeEndpoint {
                        edge: edge.id.clone(),
                        node: end.clone(),
                    });
                }
            }
        }
        for (name, value) in [
            ("rank_separation", self.config.rank_separation),
            ("node_separation", self.config.node_separation),
            ("edge_separation", self.config.edge_separation),
        ] {
            if value < 0 {
                return Err(LayoutError::BadSeparation { name, value });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str) -> Node {
        Node::new(NodeId::new(id), 120.0, 60.0)
    }

    fn edge(id: &str, from: &str, to: &str) -> Edge {
        Edge::new(EdgeId::new(id), NodeId::new(from), NodeId::new(to))
    }

    #[test]
    fn empty_graph_valid_with_reference_default_config() {
        let g = LayoutGraph::new();
        g.validate().expect("empty is valid");
        assert_eq!(
            g.config,
            LayoutConfig {
                rank_direction: RankDirection::LeftRight,
                rank_separation: 100,
                node_separation: 50,
                edge_separation: 50,
                generate_vertices: true,
            }
        );
    }

    #[test]
    fn happy_graph_validates() {
        let mut g = LayoutGraph::new();
        g.add_node(node("a"));
        g.add_node(node("b"));
        g.add_edge(edge("r", "a", "b"));
        g.validate().expect("well-formed graph");
    }

    #[test]
    fn duplicate_node_id_detected() {
        let mut g = LayoutGraph::new();
        g.add_node(node("a"));
        g.add_node(node("a"));
        assert!(matches!(
            g.validate(),
            Err(crate::LayoutError::DuplicateNodeId(id)) if id.as_str() == "a"
        ));
    }

    #[test]
    fn unknown_edge_endpoint_detected() {
        let mut g = LayoutGraph::new();
        g.add_node(node("a"));
        g.add_edge(edge("r", "a", "ghost"));
        assert!(matches!(
            g.validate(),
            Err(crate::LayoutError::UnknownEdgeEndpoint { .. })
        ));
    }

    #[test]
    fn bad_sizes_and_separations_detected() {
        let mut g = LayoutGraph::new();
        g.add_node(Node::new(NodeId::new("a"), f64::NAN, 10.0));
        assert!(matches!(
            g.validate(),
            Err(crate::LayoutError::BadNodeSize { .. })
        ));

        let mut g = LayoutGraph::new();
        g.add_node(node("a"));
        g.config.node_separation = -1;
        assert!(matches!(
            g.validate(),
            Err(crate::LayoutError::BadSeparation { value: -1, .. })
        ));
    }
}
