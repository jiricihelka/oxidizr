//! The shared Structurizr-PlantUML layer (port of
//! `export/plantuml/AbstractPlantUMLExporter`'s reusable helpers): the
//! `plantuml.*` property constants, the shape maps, the icon rules (D12
//! scale pin) and the diagram engine ([`Engine`]/[`Dialect`], the
//! `AbstractDiagramExporter` port). Dialects implement [`Dialect`]:
//! `structurizr` (M2-c), `c4plantuml` (M3).

pub mod engine;
pub mod model;
pub mod structurizr;

pub use engine::{Dialect, Engine, Frame};
pub use model::ElemRef;
pub use structurizr::StructurizrDialect;

use oxidizr_core::views::Shape;

/// `PLANTUML_TITLE_PROPERTY` (title leg gate — case-SENSITIVE `equals`).
pub const PLANTUML_TITLE_PROPERTY: &str = "plantuml.title";
/// `PLANTUML_INCLUDES_PROPERTY`.
pub const PLANTUML_INCLUDES_PROPERTY: &str = "plantuml.includes";
/// `PLANTUML_SKINPARAMS_PROPERTY`.
pub const PLANTUML_SKINPARAMS_PROPERTY: &str = "plantuml.skinparams";
/// `PLANTUML_ANIMATION_PROPERTY` (`isAnimationSupported`).
pub const PLANTUML_ANIMATION_PROPERTY: &str = "plantuml.animation";
/// `PLANTUML_SEQUENCE_DIAGRAM_PROPERTY` (dynamic-view sequence gate).
pub const PLANTUML_SEQUENCE_DIAGRAM_PROPERTY: &str = "plantuml.sequenceDiagram";
/// `PLANTUML_BOUNDARIES` (sequence-path boundary boxes).
pub const PLANTUML_BOUNDARIES_PROPERTY: &str = "plantuml.boundaries";
/// Element-style property `plantuml.shadow` (`StructurizrPlantUMLExporter`).
pub const PLANTUML_SHADOW_PROPERTY: &str = "plantuml.shadow";
/// `DIAGRAM_TITLE_TAG` (title font-size style tag).
pub const DIAGRAM_TITLE_TAG: &str = "Diagram:Title";
/// `DIAGRAM_DESCRIPTION_TAG`.
pub const DIAGRAM_DESCRIPTION_TAG: &str = "Diagram:Description";

/// D12: the icon scale is PINNED to the reference's fallback constant
/// (`calculateIconScale` fetches the image; every failure path lands here
/// — oxidizr never touches the network during export). Already formatted
/// as the reference would render it (`Double.toString(0.5)`).
pub const ICON_SCALE: &str = "0.5";

/// `AbstractPlantUMLExporter.isSupportedIcon`: non-empty (trim) AND the
/// raw string starts with `http` (any http prefix — not https-only).
#[must_use]
pub fn is_supported_icon(icon: Option<&str>) -> bool {
    icon.is_some_and(|i| !i.trim().is_empty() && i.starts_with("http"))
}

/// `plantUMLShapeOf(Shape)` (catalog §4).
#[must_use]
pub const fn plantuml_shape_of(shape: Option<Shape>) -> &'static str {
    match shape {
        Some(Shape::Person | Shape::Robot) => "person",
        Some(Shape::Component) => "component",
        Some(Shape::Cylinder) => "database",
        Some(Shape::Folder) => "folder",
        Some(Shape::Ellipse | Shape::Circle) => "storage",
        Some(Shape::Hexagon) => "hexagon",
        Some(Shape::Pipe) => "queue",
        _ => "rectangle",
    }
}

/// `plantumlSequenceType(Shape)` (catalog §8).
#[must_use]
pub const fn plantuml_sequence_shape_of(shape: Option<Shape>) -> &'static str {
    match shape {
        Some(Shape::Person) => "actor",
        Some(Shape::Cylinder) => "database",
        Some(Shape::Folder) => "collections",
        Some(Shape::Ellipse | Shape::Circle) => "entity",
        _ => "participant",
    }
}

/// Java `String.compareTo` (UTF-16 code units) — the ordering behind every
/// name/id/group sort in the engine (same precedent as core `viewops`).
pub(crate) fn java_cmp_str(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}
