//! `oxidizr-layout` — the pure graph + layered layout engine.
//!
//! * Neutral graph types in, placements out: [`LayoutGraph`] (nodes with
//!   measured sizes, edges with optional endpoint anchors, [`LayoutConfig`]) →
//!   [`LayoutResult`] (node positions, edge polylines whose interior points
//!   are the waypoints).
//! * [`LayoutEngine`] is the seam: the own [`Layered`] engine implements
//!   it here; the optional `Dot` subprocess behind the `dot-backend`
//!   feature shells out to `$OXIDZR_DOT_BIN`. Neither this crate nor its
//!   types may ever know about `oxidizr-core`, IO or GUI — the viewer
//!   maps domain ids in and geometry out (excepting the `dot-backend`
//!   feature itself).
//!
//! **Determinism is a hard contract**: same input ⇒ byte-identical
//! output. No randomness, stable sorts only, fixed iteration bounds —
//! every engine carries a property test for it.
//!
//! Zero dependencies, not even `thiserror`: [`LayoutError`] hand-rolls
//! `Display` + [`std::error::Error`].

#[cfg(feature = "dot-backend")]
mod dot;
mod engine;
mod graph;
mod layered;

#[cfg(feature = "dot-backend")]
pub use dot::{COMPONENT_GAP_PX, Dot, PX_PER_INCH};
pub use engine::{EdgePlacement, LayoutEngine, LayoutError, LayoutResult, NodePlacement, Point};
pub use graph::{Anchor, Edge, EdgeId, LayoutConfig, LayoutGraph, Node, NodeId, RankDirection};
pub use layered::Layered;
