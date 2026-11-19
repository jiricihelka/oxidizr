//! `oxidizr-layout` — the pure graph + layered layout engine (P3-D1, P3-D5 of
//! `agents/plan/build/PHASE_3.md`).
//!
//! * Neutral graph types in, placements out: [`LayoutGraph`] (nodes with
//!   measured sizes, edges with optional endpoint anchors, [`LayoutConfig`]) →
//!   [`LayoutResult`] (node positions, edge polylines whose interior points
//!   are the waypoints).
//! * [`LayoutEngine`] is the seam: the implementations arrive in P3-M2 — the
//!   own `Layered` engine, plus an optional `Dot` subprocess behind the
//!   `dot-backend` feature (P3-D5). Neither this crate nor its types may ever
//!   know about `oxidizr-core`, IO or GUI — the viewer maps domain ids in and
//!   geometry out.
//!
//! **Determinism is a hard contract** (P3-D5): same input ⇒ byte-identical
//! output. No randomness, stable sorts only, fixed iteration bounds — every
//! engine carries a property test for it (`M2`).
//!
//! Zero dependencies, not even `thiserror`: the P3-D1 promise beats the
//! error-type convention here ([`LayoutError`] hand-rolls `Display` +
//! [`std::error::Error`]).

mod engine;
mod graph;

pub use engine::{EdgePlacement, LayoutEngine, LayoutError, LayoutResult, NodePlacement, Point};
pub use graph::{Anchor, Edge, EdgeId, LayoutConfig, LayoutGraph, Node, NodeId, RankDirection};
