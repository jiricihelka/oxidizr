//! The shared Structurizr-PlantUML layer: the `plantuml.*` property
//! constants, the shape maps, the icon rules and the diagram engine
//! ([`Engine`]/[`Dialect`]). Dialects implement [`Dialect`]:
//! `structurizr` and `c4plantuml`.

pub mod c4;
pub mod engine;
pub mod model;
pub mod structurizr;

pub(crate) use c4::C4PlantUmlDialect;
pub use engine::{Dialect, Engine, Frame};
pub use model::ElemRef;
pub(crate) use structurizr::StructurizrDialect;

use oxidizr_core::views::Shape;

/// `plantuml.title` — title gate; the match is case-SENSITIVE.
pub(crate) const PLANTUML_TITLE_PROPERTY: &str = "plantuml.title";
/// `plantuml.includes` — comma-separated `!include` lines.
pub(crate) const PLANTUML_INCLUDES_PROPERTY: &str = "plantuml.includes";
/// `plantuml.skinparams` — comma-separated `name=value` pairs written into
/// a `skinparam { … }` block.
pub(crate) const PLANTUML_SKINPARAMS_PROPERTY: &str = "plantuml.skinparams";
/// `plantuml.animation` — whether animation frames are exported.
pub(crate) const PLANTUML_ANIMATION_PROPERTY: &str = "plantuml.animation";
/// `plantuml.sequenceDiagram` — sequence-diagram gate for dynamic views.
pub(crate) const PLANTUML_SEQUENCE_DIAGRAM_PROPERTY: &str = "plantuml.sequenceDiagram";
/// `plantuml.boundaries` — boundary boxes along sequence paths.
pub(crate) const PLANTUML_BOUNDARIES_PROPERTY: &str = "plantuml.boundaries";
/// Element-style property `plantuml.shadow`.
pub(crate) const PLANTUML_SHADOW_PROPERTY: &str = "plantuml.shadow";
/// `c4plantuml.legend` — controls `SHOW_LEGEND` (default `true`).
pub(crate) const C4PLANTUML_LEGEND_PROPERTY: &str = "c4plantuml.legend";
/// `c4plantuml.stereotypes` — `show`/`hide stereotypes` (default `false`).
pub(crate) const C4PLANTUML_STEREOTYPES_PROPERTY: &str = "c4plantuml.stereotypes";
/// `c4plantuml.tags` — includes the tags section (default `false`).
pub(crate) const C4PLANTUML_TAGS_PROPERTY: &str = "c4plantuml.tags";
/// `c4plantuml.stdlib` — `<C4/…>` standard-library includes vs raw-URL
/// includes (default `true`).
pub(crate) const C4PLANTUML_STANDARD_LIBRARY_PROPERTY: &str = "c4plantuml.stdlib";
/// Element-style property `c4plantuml.sprite` (tags section `$sprite`).
pub(crate) const C4PLANTUML_SPRITE: &str = "c4plantuml.sprite";
/// Element-style property `c4plantuml.shadow` (tags section `$shadowing`).
pub(crate) const C4PLANTUML_SHADOW: &str = "c4plantuml.shadow";
/// `c4plantuml.elementProperties` — writes `AddProperty` lines on elements
/// and deployment nodes (default `false`).
pub(crate) const C4PLANTUML_ELEMENT_PROPERTIES_PROPERTY: &str = "c4plantuml.elementProperties";
/// `c4plantuml.relationshipProperties` — writes `AddProperty` lines on
/// relationships (default `false`).
pub(crate) const C4PLANTUML_RELATIONSHIP_PROPERTIES_PROPERTY: &str =
    "c4plantuml.relationshipProperties";
/// `Diagram:Title` — element style tag giving the diagram title its style.
pub(crate) const DIAGRAM_TITLE_TAG: &str = "Diagram:Title";
/// `Diagram:Description` — element style tag giving the diagram description
/// its style.
pub(crate) const DIAGRAM_DESCRIPTION_TAG: &str = "Diagram:Description";

/// The icon scale is PINNED to the official tooling's fallback constant
/// (the icon fetch always fails over to it — oxidizr never touches the
/// network during export, so the fallback always applies). Already
/// formatted the way the official tooling renders it.
pub(crate) const ICON_SCALE: &str = "0.5";

/// An icon is supported when its URL is non-empty (after trimming) AND the
/// raw string starts with `http` (any http prefix — not https-only).
#[must_use]
pub(crate) fn is_supported_icon(icon: Option<&str>) -> bool {
    icon.is_some_and(|i| !i.trim().is_empty() && i.starts_with("http"))
}

/// The `PlantUML` shape name for a model shape.
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

/// The `PlantUML` sequence-diagram participant type for a model shape.
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

/// The UTF-16 code-unit text order — the ordering behind
/// every name/id/group sort in the engine (same precedent as core `viewops`).
pub(crate) fn java_cmp_str(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}
