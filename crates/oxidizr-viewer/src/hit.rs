//! Hand-rolled hit-testing (P3-D3): AABB + point-to-segment over the
//! [`oxidizr-viewer::scene`](crate::scene) — no geometry crate.
//!
//! M0 boundary stub: lands with M1 selection.

/// Placeholder for a hit result (M1).
#[derive(Debug)]
pub enum Hit {
    /// An element of the given index in its layer.
    Element(usize),
    /// An edge of the given index.
    Edge(usize),
}
