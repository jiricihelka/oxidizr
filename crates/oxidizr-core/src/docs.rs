//! Workspace documentation: sections, decisions and images. The
//! `!docs`/`!adrs` importers are shipped; the model types are complete so
//! JSON round-trips fully.
//!
//! Field truth (library 6.2.4): `Section` = `{filename, order,
//! elementId?, content, format}` plus a BACK-COMPAT `title` that always
//! serializes as `""` (always written);
//! no `id`/`language`/nested `sections`/`links` on sections (legacy keys in
//! older goldens are read-ignored). `Decision` = `{id, title, date, status,
//! links, elementId?, content, format}` — the rich `options`/`rationale`/
//! `decisionStatus` shape lives in the decisions import module, not
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

impl Format {
    /// JSON spelling (the official constant name).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Format::Markdown => "Markdown",
            Format::AsciiDoc => "AsciiDoc",
        }
    }

    /// Parse the JSON spelling (exact case).
    #[must_use]
    pub fn parse(s: &str) -> Option<Format> {
        match s {
            "Markdown" => Some(Format::Markdown),
            "AsciiDoc" => Some(Format::AsciiDoc),
            _ => None,
        }
    }
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
    /// Content format — nullable bean field (unknown/absent JSON `format`
    /// binds null; omitted on write).
    pub format: Option<Format>,
    /// Element this section is attached to (JSON `elementId`).
    pub element_id: Option<Id>,
}

/// A link between decisions (`documentation/Decision.Link`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// Linked decision id.
    pub id: String,
    /// Description (defaults to `""`).
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
    /// Decision date, epoch SECONDS (the JSON
    /// writer formats `yyyy-MM-dd'T'HH:mm:ss'Z'` UTC — sub-second precision is
    /// dropped by the format).
    pub date: i64,
    /// Links to other decisions.
    pub links: Vec<Link>,
    /// Markdown/AsciiDoc content (the decision body).
    pub content: String,
    /// Content format — nullable bean field (see [`Section::format`]).
    pub format: Option<Format>,
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
    /// Content type (exact JSON key `type`,
    /// pinned by the goldens).
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
