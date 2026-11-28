//! The engine seam ([`LayoutEngine`]), the output placements and the error
//! type.

use std::fmt;

use crate::graph::{EdgeId, NodeId};

/// A point in layout space (px). Placements are floats; consumers round when
/// they write integer JSON geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// X coordinate (px).
    pub x: f64,
    /// Y coordinate (px).
    pub y: f64,
}

/// One node's position (top-left of its box, px).
#[derive(Debug, Clone, PartialEq)]
pub struct NodePlacement {
    /// Which node.
    pub id: NodeId,
    /// Top-left position.
    pub position: Point,
}

/// One edge's route. The first/last points are the endpoints (honoring the
/// graph's [`Anchor`](crate::Anchor)s when set); the interior points are the
/// waypoints (empty when straight or `generate_vertices` is false).
#[derive(Debug, Clone, PartialEq)]
pub struct EdgePlacement {
    /// Which edge.
    pub id: EdgeId,
    /// Route polyline, source → target, at least the two endpoints.
    pub points: Vec<Point>,
}

/// The result of one [`LayoutEngine::layout`] run: every graph node and edge
/// exactly once, in the graph's input order (the determinism contract).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayoutResult {
    /// Node placements.
    pub nodes: Vec<NodePlacement>,
    /// Edge placements.
    pub edges: Vec<EdgePlacement>,
}

/// The one error type — std-only `Display`/`Error` (zero-dep promise,
/// deliberately not `thiserror`).
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutError {
    /// Two nodes carry the same id.
    DuplicateNodeId(NodeId),
    /// An edge names a node the graph does not contain.
    UnknownEdgeEndpoint {
        /// The offending edge.
        edge: EdgeId,
        /// The unresolvable endpoint.
        node: NodeId,
    },
    /// A node size is non-finite or negative.
    BadNodeSize {
        /// The offending node.
        id: NodeId,
        /// Reported width.
        width: f64,
        /// Reported height.
        height: f64,
    },
    /// A config separation is negative.
    BadSeparation {
        /// Config field name.
        name: &'static str,
        /// Reported value.
        value: i32,
    },
    /// Layering did not converge after cycle removal. Defensive only:
    /// the DFS reversal guarantees a DAG, so reaching this means an
    /// engine bug, never bad input.
    CycleRemovalFailed,
    /// `dot-backend` only: the external `dot` binary could not be
    /// spawned, exited non-zero, or emitted `-Tplain` output this
    /// parser rejects.
    #[cfg(feature = "dot-backend")]
    DotBackend(String),
    /// `dagre` only: the pipeline failed to place a valid graph — a node
    /// label or edge route went missing, or a coordinate came back
    /// non-finite. The graph itself passed validation, so this is always
    /// the engine failing, never the input.
    #[cfg(feature = "dagre")]
    Dagre(String),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateNodeId(id) => write!(f, "duplicate node id: {id}"),
            Self::UnknownEdgeEndpoint { edge, node } => {
                write!(f, "edge {edge} references unknown node {node}")
            }
            Self::BadNodeSize { id, width, height } => {
                write!(f, "node {id} has a bad size: {width} x {height}")
            }
            Self::BadSeparation { name, value } => {
                write!(f, "config separation {name} must be >= 0, got {value}")
            }
            Self::CycleRemovalFailed => write!(f, "layering did not converge"),
            #[cfg(feature = "dot-backend")]
            Self::DotBackend(message) => write!(f, "dot backend: {message}"),
            #[cfg(feature = "dagre")]
            Self::Dagre(message) => write!(f, "dagre: {message}"),
        }
    }
}

impl std::error::Error for LayoutError {}

/// A layout implementation over the neutral graph. The
/// **determinism contract is hard**: same input ⇒ identical output — no
/// randomness, stable sorts only, fixed iteration bounds. Implementations
/// must run [`crate::LayoutGraph::validate`] first and reject invalid input.
pub trait LayoutEngine {
    /// Compute placements for `graph`.
    ///
    /// # Errors
    ///
    /// [`LayoutError`] when the graph fails structural validation, or the
    /// engine cannot route it.
    fn layout(&self, graph: &crate::LayoutGraph) -> Result<LayoutResult, LayoutError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_display() {
        let e = LayoutError::UnknownEdgeEndpoint {
            edge: EdgeId::new("r1"),
            node: NodeId::new("ghost"),
        };
        assert_eq!(e.to_string(), "edge r1 references unknown node ghost");
        let d = LayoutError::DuplicateNodeId(NodeId::new("a"));
        assert_eq!(d.to_string(), "duplicate node id: a");
    }

    #[test]
    fn trait_is_object_safe() {
        struct Straight;
        impl LayoutEngine for Straight {
            fn layout(&self, graph: &crate::LayoutGraph) -> Result<LayoutResult, LayoutError> {
                graph.validate()?;
                Ok(LayoutResult {
                    nodes: {
                        let mut x = 0.0;
                        graph
                            .nodes
                            .iter()
                            .map(|n| {
                                let p = NodePlacement {
                                    id: n.id.clone(),
                                    position: Point { x, y: 0.0 },
                                };
                                x += 10.0;
                                p
                            })
                            .collect()
                    },
                    edges: graph
                        .edges
                        .iter()
                        .map(|e| EdgePlacement {
                            id: e.id.clone(),
                            points: Vec::new(),
                        })
                        .collect(),
                })
            }
        }
        let engine: Box<dyn LayoutEngine> = Box::new(Straight);
        let result = engine.layout(&crate::LayoutGraph::new()).expect("empty");
        assert_eq!(result, LayoutResult::default());
    }
}
