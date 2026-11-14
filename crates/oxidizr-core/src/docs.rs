//! Workspace documentation (`com.structurizr.documentation`): sections,
//! decisions and images. The `!docs`/`!adrs` IMPORTERS land in Phase 4 (D7);
//! the model types are complete now so JSON round-trips fully.
//!
//! v6.2.4 field truth (verified @ `ad34036`): `Section` = `{filename, order,
//! elementId?, content, format}` plus a BACK-COMPAT `title` that always
//! serializes as `""` (`@JsonGetter @JsonInclude(ALWAYS)` — write-catalog item);
//! no `id`/`language`/nested `sections`/`links` on sections (legacy keys in
//! older goldens are read-ignored). `Decision` = `{id, title, date, status,
//! links, elementId?, content, format}` — the rich `options`/`rationale`/
//! `decisionStatus` shape lives in the decisions IMPORT module (Phase 4), not
//! the core model.

use crate::ids::Id;

/// Content format (`documentation/Format`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    /// Markdown.
    #[default]
    Markdown,
    /// `AsciiDoc`.
    AsciiDoc,
}

/// A documentation section (`documentation/Section`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// Sort order.
    pub order: i32,
    /// Source filename.
    pub filename: Option<String>,
    /// Markdown/AsciiDoc content.
    pub content: String,
    /// Content format.
    pub format: Format,
    /// Element this section is attached to (JSON `elementId`).
    pub element_id: Option<Id>,
}

/// A link between decisions (`documentation/Decision.Link`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// Linked decision id.
    pub id: String,
    /// Description (Java default `""`).
    pub description: String,
}

/// An architecture decision (`documentation/Decision`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// Decision id ("1").
    pub id: String,
    /// Decision title.
    pub title: String,
    /// Status.
    pub status: String,
    /// Decision date, epoch SECONDS (reference `java.util.Date`; the JSON
    /// writer formats `yyyy-MM-dd'T'HH:mm:ss'Z'` UTC — sub-second precision is
    /// dropped exactly like the reference format).
    pub date: i64,
    /// Links to other decisions.
    pub links: Vec<Link>,
    /// Markdown/AsciiDoc content (the decision body).
    pub content: String,
    /// Content format.
    pub format: Format,
    /// Element this decision is attached to (JSON `elementId`).
    pub element_id: Option<Id>,
}

/// A documentation image (`documentation/Image` — v6.2.4 has exactly
/// `{name, content, type}`; other keys are read-ignored legacy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Image name.
    pub name: String,
    /// Base64/data-URI content.
    pub content: String,
    /// Content type (Java field `type`; exact JSON key pinned by the M3 write
    /// catalog).
    pub content_type: Option<String>,
}

/// The documentation tree (`documentation/Documentation`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Documentation {
    /// Sections.
    pub sections: Vec<Section>,
    /// Decisions.
    pub decisions: Vec<Decision>,
    /// Images.
    pub images: Vec<Image>,
}
