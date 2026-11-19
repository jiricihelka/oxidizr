//! The retained scene (P3-D3): `Workspace` + resolved styles →
//! [`Scene`] layers/items/edges, rebuilt only on model change, layout edit
//! or zoom-band transition — never per frame.
//!
//! M0 boundary stub: the scene model itself lands in M1 together with the
//! first open/pan/zoom path.

/// Placeholder for the retained scene graph (M1).
#[derive(Debug, Default)]
pub struct Scene;
