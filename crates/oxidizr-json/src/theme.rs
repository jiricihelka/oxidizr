//! `theme.json` de/serialization — the `view/Theme` bean behind
//! `ThemeUtils.toJson` / `ThemeUtils.fromJson` (Phase 2 `export --format
//! theme`, P2-D4; theme loading is M1).
//!
//! The reference uses a PLAIN `ObjectMapper` (not the workspace mapper): key
//! order is the Java declaration order, the mapper inclusion is `NON_EMPTY`
//! (theme fields and style arrays), style fields carry per-field
//! `@JsonInclude(NON_NULL)` (so `0`/`false`/`""` survive), while `tag` and
//! `properties` follow `AbstractStyle`'s `NON_EMPTY`. Witnesses:
//! `/tmp/nix-shell.uni100/p2/v1/out3,out6` (oracle probes 2026-11-17).
//! Indentation is the same `DefaultPrettyPrinter` emulation as the workspace
//! writer (§0) — the reference configures both identically.

use oxidizr_core::Workspace;
use oxidizr_core::views::{ElementStyle, RelationshipStyle, Theme};

use crate::error::JsonError;
use crate::printer::{Json, to_bytes};
use crate::reader;
use crate::writer::opt_nnn;

/// Deserialize a `theme.json` document with the `ThemeUtils.fromJson`
/// leniency matrix (unknown keys ignored, `ACCEPT_EMPTY_STRING_AS_NULL_OBJECT`
/// — the style-item binding is the workspace one, clamps included).
///
/// # Errors
///
/// [`JsonError::Parse`] for malformed JSON, [`JsonError::Read`] for a
/// non-object root or a style value the reference setters reject (invalid
/// color), [`JsonError::Validation`] never (themes are not validated).
pub fn theme_from_slice(bytes: &[u8]) -> Result<Theme, JsonError> {
    let root: serde_json::Value = serde_json::from_slice(bytes)?;
    let obj = root
        .as_object()
        .ok_or_else(|| JsonError::Read("the theme JSON must be an object".to_string()))?;
    Ok(Theme {
        name: reader::text(obj, "name"),
        description: reader::text(obj, "description"),
        elements: reader::objects(obj, "elements")
            .into_iter()
            .map(reader::read_element_style)
            .collect::<Result<Vec<ElementStyle>, JsonError>>()?,
        relationships: reader::objects(obj, "relationships")
            .into_iter()
            .map(reader::read_relationship_style)
            .collect::<Result<Vec<RelationshipStyle>, JsonError>>()?,
        license: reader::text(obj, "license"),
    })
}

/// Serialize the `Theme` bean exactly as `ThemeUtils.toJson` writes it.
#[must_use]
pub fn theme_write(theme: &Theme) -> Vec<u8> {
    to_bytes(&theme_json(&ThemeView::of(theme)))
}

/// Borrowed `Theme` for serialization (2R R-mis-08): `theme_to_vec` feeds
/// the live workspace fields directly; `theme_write` borrows a bean.
struct ThemeView<'a> {
    name: Option<&'a str>,
    description: Option<&'a str>,
    elements: &'a [ElementStyle],
    relationships: &'a [RelationshipStyle],
    license: Option<&'a str>,
}

impl ThemeView<'_> {
    /// Borrow a loaded `Theme` bean (the `theme_write` path).
    fn of(theme: &Theme) -> ThemeView<'_> {
        ThemeView {
            name: theme.name.as_deref(),
            description: theme.description.as_deref(),
            elements: &theme.elements,
            relationships: &theme.relationships,
            license: theme.license.as_deref(),
        }
    }
}

/// The `export --format theme` payload: `ThemeUtils.toJson(workspace)` —
/// name/description from the workspace (`NON_EMPTY`), the LIVE style
/// collections in storage order (sorted for DSL-built workspaces, document
/// order for JSON-loaded ones — §5), `license` never set.
#[must_use]
pub fn theme_to_vec(workspace: &Workspace) -> Vec<u8> {
    // 2R R-mis-08: the LIVE workspace fields serialize through a borrowed
    // view — the old shape deep-cloned name/description/both style vectors
    // into a throwaway `Theme` for one serialize pass.
    let styles = &workspace.views.configuration.styles;
    to_bytes(&theme_json(&ThemeView {
        name: workspace.name.as_deref(),
        description: workspace.description.as_deref(),
        elements: &styles.element_styles,
        relationships: &styles.relationship_styles,
        license: None,
    }))
}

fn theme_json(t: &ThemeView<'_>) -> Json {
    // `Theme` declaration order (name, description, elements, relationships,
    // license); NON_EMPTY hides null/"" and empty arrays (a theme with no
    // styles serializes as `{ }` — witness out1-era probe).
    let mut entries = vec![
        ("name", Json::opt_text(t.name)),
        ("description", Json::opt_text(t.description)),
    ];
    if !t.elements.is_empty() {
        entries.push((
            "elements",
            Some(Json::Arr(
                t.elements.iter().map(element_style).collect::<Vec<_>>(),
            )),
        ));
    }
    if !t.relationships.is_empty() {
        entries.push((
            "relationships",
            Some(Json::Arr(
                t.relationships.iter().map(relationship_style).collect(),
            )),
        ));
    }
    entries.push(("license", Json::opt_text(t.license)));
    Json::obj(entries)
}

/// Element style in `ElementStyle.java` declaration order: the parent fields
/// first (`tag`, `colorScheme`, then `AbstractPropertyContainer.properties` —
/// witness order 2026-11-17), then width…description.
fn element_style(s: &ElementStyle) -> Json {
    Json::obj(vec![
        ("tag", Json::text(&s.tag)),
        (
            "colorScheme",
            s.color_scheme.map(|c| Json::Str(c.as_str().to_string())),
        ),
        ("properties", Json::props(&s.properties)),
        ("width", s.width.map(|x| Json::Num(i64::from(x)))),
        ("height", s.height.map(|x| Json::Num(i64::from(x)))),
        ("background", opt_nnn(s.background.as_deref())),
        ("stroke", opt_nnn(s.stroke.as_deref())),
        (
            "strokeWidth",
            s.stroke_width.map(|x| Json::Num(i64::from(x))),
        ),
        ("color", opt_nnn(s.color.as_deref())),
        ("fontSize", s.font_size.map(|x| Json::Num(i64::from(x)))),
        ("shape", s.shape.map(|x| Json::Str(x.as_str().to_string()))),
        ("icon", opt_nnn(s.icon.as_deref())),
        (
            "iconPosition",
            s.icon_position.map(|x| Json::Str(x.as_str().to_string())),
        ),
        (
            "border",
            s.border.map(|x| Json::Str(x.as_str().to_string())),
        ),
        ("opacity", s.opacity.map(|x| Json::Num(i64::from(x)))),
        ("metadata", s.metadata.map(Json::Bool)),
        ("description", s.description.map(Json::Bool)),
    ])
}

/// Relationship style: `tag`, `colorScheme`, `properties`, then
/// `thickness, color, fontSize, width, dashed, style, routing, jump,
/// position, opacity, metadata, description` (declaration order).
fn relationship_style(s: &RelationshipStyle) -> Json {
    Json::obj(vec![
        ("tag", Json::text(&s.tag)),
        (
            "colorScheme",
            s.color_scheme.map(|c| Json::Str(c.as_str().to_string())),
        ),
        ("properties", Json::props(&s.properties)),
        ("thickness", s.thickness.map(|x| Json::Num(i64::from(x)))),
        ("color", opt_nnn(s.color.as_deref())),
        ("fontSize", s.font_size.map(|x| Json::Num(i64::from(x)))),
        ("width", s.width.map(|x| Json::Num(i64::from(x)))),
        ("dashed", s.dashed.map(Json::Bool)),
        ("style", s.style.map(|x| Json::Str(x.as_str().to_string()))),
        (
            "routing",
            s.routing.map(|x| Json::Str(x.as_str().to_string())),
        ),
        ("jump", s.jump.map(Json::Bool)),
        ("position", s.position.map(|x| Json::Num(i64::from(x)))),
        ("opacity", s.opacity.map(|x| Json::Num(i64::from(x)))),
        ("metadata", s.metadata.map(Json::Bool)),
        ("description", s.description.map(Json::Bool)),
    ])
}
