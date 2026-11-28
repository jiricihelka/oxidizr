//! Neutral graph input types ([`LayoutGraph`] and friends).
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

/// Caller-assigned compound-group identity (opaque here).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GroupId(String);

impl GroupId {
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

impl fmt::Display for GroupId {
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

/// Compound containment: one container of nodes (a viewer boundary box
/// entering the layout as a parent). `members` are the container's DIRECT
/// nodes; nesting is expressed through `parent` (containment is a forest —
/// every node sits in at most one group, every group has at most one
/// parent). Engines that do not support containment ignore groups
/// ([`Layered`](crate::Layered) and `Dot` lay exactly the node set); a
/// supporting engine keeps the members together as one unit.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    /// Identity (independent namespace from nodes and edges).
    pub id: GroupId,
    /// Enclosing group (`None` = top level).
    pub parent: Option<GroupId>,
    /// Member nodes, in caller order.
    pub members: Vec<NodeId>,
}

impl Group {
    /// Top-level group with the given members.
    #[must_use]
    pub fn new(id: GroupId, members: Vec<NodeId>) -> Self {
        Self {
            id,
            parent: None,
            members,
        }
    }
}

/// Rank direction — the four `automaticLayout` directions (the viewer maps
/// `oxidizr-core`'s enum; names mirror it so the mapping is trivial).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RankDirection {
    TopDown,
    BottomUp,
    /// The default: left-right (matches `oxidizr-core`'s
    /// `AutomaticLayout::default`).
    #[default]
    LeftRight,
    RightLeft,
}

/// Engine configuration (auto-layout parameters). Defaults: rank 100,
/// node 50, edge 50, vertices on.
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

/// One layout input: nodes, edges, compound groups and config, in caller
/// (deterministic) order. Build with [`LayoutGraph::new`] +
/// [`add_node`](Self::add_node) + [`add_edge`](Self::add_edge) +
/// [`add_group`](Self::add_group); engines call
/// [`validate`](Self::validate) themselves.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayoutGraph {
    /// Nodes in insertion order.
    pub nodes: Vec<Node>,
    /// Edges in insertion order.
    pub edges: Vec<Edge>,
    /// Compound groups in insertion order (engines without containment
    /// support lay the node set as if this were empty).
    pub groups: Vec<Group>,
    /// Engine configuration.
    pub config: LayoutConfig,
}

impl LayoutGraph {
    /// Empty graph with the default [`LayoutConfig`].
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

    /// Append a compound group.
    pub fn add_group(&mut self, group: Group) {
        self.groups.push(group);
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
        self.validate_groups(&seen)
    }

    /// Containment is a forest: unique group ids, resolvable members and
    /// parents, every node in at most one group, acyclic parent links.
    fn validate_groups(
        &self,
        node_ids: &std::collections::HashSet<&str>,
    ) -> Result<(), crate::LayoutError> {
        use crate::LayoutError;
        use std::collections::{HashMap, HashSet};

        let mut seen_groups = HashSet::new();
        let mut parent_of: HashMap<&str, Option<&str>> = HashMap::new();
        for group in &self.groups {
            if !seen_groups.insert(group.id.as_str()) {
                return Err(LayoutError::DuplicateGroupId(group.id.clone()));
            }
            for member in &group.members {
                if !node_ids.contains(member.as_str()) {
                    return Err(LayoutError::UnknownGroupMember {
                        group: group.id.clone(),
                        node: member.clone(),
                    });
                }
            }
            parent_of.insert(
                group.id.as_str(),
                group.parent.as_ref().map(GroupId::as_str),
            );
        }
        let mut owned: HashSet<&str> = HashSet::new();
        for group in &self.groups {
            for member in &group.members {
                if !owned.insert(member.as_str()) {
                    return Err(LayoutError::DuplicateGroupMembership(member.clone()));
                }
            }
        }
        for group in &self.groups {
            if let Some(parent) = group.parent.as_ref()
                && !parent_of.contains_key(parent.as_str())
            {
                return Err(LayoutError::UnknownGroupParent {
                    group: group.id.clone(),
                    parent: parent.clone(),
                });
            }
        }
        // Cycle check: walk each parent chain with a fixed bound — a chain
        // of `groups.len()` links must end at `None`; continuing past that
        // means revisiting a group.
        for group in &self.groups {
            let mut cursor = group.parent.as_ref().map(GroupId::as_str);
            let mut steps = 0usize;
            while let Some(id) = cursor {
                if steps > self.groups.len() {
                    return Err(LayoutError::GroupParentCycle {
                        group: group.id.clone(),
                    });
                }
                steps += 1;
                cursor = parent_of[id];
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

    fn group(id: &str, parent: Option<&str>, members: &[&str]) -> Group {
        Group {
            id: GroupId::new(id),
            parent: parent.map(GroupId::new),
            members: members.iter().map(|m| NodeId::new(*m)).collect(),
        }
    }

    #[test]
    fn valid_group_forest_accepted() {
        let mut g = LayoutGraph::new();
        for id in ["a", "b", "c"] {
            g.add_node(node(id));
        }
        g.add_group(group("outer", None, &["c"]));
        g.add_group(group("inner", Some("outer"), &["a", "b"]));
        assert_eq!(g.validate(), Ok(()));
    }

    #[test]
    fn group_invariants_detected() {
        let mut g = LayoutGraph::new();
        g.add_node(node("a"));
        g.add_node(node("b"));
        g.add_group(group("g1", None, &["a"]));
        g.add_group(group("g1", None, &["b"]));
        assert!(matches!(
            g.validate(),
            Err(crate::LayoutError::DuplicateGroupId(id)) if id.as_str() == "g1"
        ));

        let mut g = LayoutGraph::new();
        g.add_node(node("a"));
        g.add_group(group("g1", None, &["ghost"]));
        assert!(matches!(
            g.validate(),
            Err(crate::LayoutError::UnknownGroupMember { .. })
        ));

        let mut g = LayoutGraph::new();
        g.add_node(node("a"));
        g.add_group(group("g1", None, &["a"]));
        g.add_group(group("g2", None, &["a"]));
        assert!(matches!(
            g.validate(),
            Err(crate::LayoutError::DuplicateGroupMembership(id)) if id.as_str() == "a"
        ));

        let mut g = LayoutGraph::new();
        g.add_group(group("g1", Some("ghost"), &[]));
        assert!(matches!(
            g.validate(),
            Err(crate::LayoutError::UnknownGroupParent { .. })
        ));

        let mut g = LayoutGraph::new();
        g.add_group(group("g1", Some("g2"), &[]));
        g.add_group(group("g2", Some("g1"), &[]));
        assert!(matches!(
            g.validate(),
            Err(crate::LayoutError::GroupParentCycle { .. })
        ));
    }

    #[test]
    fn group_self_parent_is_a_cycle() {
        let mut g = LayoutGraph::new();
        g.add_group(group("g1", Some("g1"), &[]));
        assert!(matches!(
            g.validate(),
            Err(crate::LayoutError::GroupParentCycle { .. })
        ));
    }
}
