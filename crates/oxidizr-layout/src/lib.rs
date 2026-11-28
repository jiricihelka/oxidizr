//! `oxidizr-layout` — the pure graph + layered layout engines.
//!
//! * Neutral graph types in, placements out: [`LayoutGraph`] (nodes with
//!   measured sizes, edges with optional endpoint anchors, [`LayoutConfig`]) →
//!   [`LayoutResult`] (node positions, edge polylines whose interior points
//!   are the waypoints).
//! * [`LayoutEngine`] is the seam, and every engine is a feature + runtime
//!   availability question: [`Layered`] is always there (the own engine);
//!   `Dagre` joins it behind the `dagre` cargo feature (layered layout with
//!   network-simplex ranking, Brandes-Köpf coordinates and compound
//!   graphs); `Dot` behind `dot-backend` (a `graphviz` subprocess reading
//!   `$OXIDZR_DOT_BIN`). Neither this crate nor its types may ever know
//!   about `oxidizr-core`, IO or GUI — the viewer maps domain ids in and
//!   geometry out (excepting those two optional engines themselves).
//!
//! **Determinism is a hard contract**: same input ⇒ byte-identical
//! output. No randomness, stable sorts only, fixed iteration bounds —
//! every engine carries a property test for it.
//!
//! The default feature set has no dependencies at all, not even
//! `thiserror`: [`LayoutError`] hand-rolls `Display` + [`std::error::Error`].
//! `dagre` adds `dagre` + `log` + `serde` + `serde_json` — all pure
//! computation, no IO and no windowing; `dot-backend` adds no dependency,
//! only `std::process` use.

#[cfg(feature = "dagre")]
mod dagre;
#[cfg(feature = "dot-backend")]
mod dot;
mod engine;
mod graph;
mod layered;

#[cfg(feature = "dagre")]
pub use dagre::Dagre;
#[cfg(feature = "dot-backend")]
pub use dot::Dot;
pub use engine::{EdgePlacement, LayoutEngine, LayoutError, LayoutResult, NodePlacement, Point};
pub use graph::{Anchor, Edge, EdgeId, LayoutConfig, LayoutGraph, Node, NodeId, RankDirection};
pub use layered::Layered;
