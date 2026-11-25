//! Free engine legs of the Structurizr dialect: the `getAutomaticLayout`
//! accessor, the color/defaults readers, the style-attribute helpers and
//! the official id/display-name resolution legs the dialects share. The
//! dialect hooks live in the parent module; the seven renderers in `style`.

use oxidizr_core::Id;
use oxidizr_core::styles::{
    DEFAULT_BACKGROUND_DARK, DEFAULT_BACKGROUND_LIGHT, DEFAULT_COLOR_DARK, DEFAULT_COLOR_LIGHT,
    instance_tag_chain,
};
use oxidizr_core::validate::ElementKind;
use oxidizr_core::views::{AutomaticLayout, ColorScheme, ElementStyle, LineStyle, View};

use crate::error::ExportError;
use crate::plantuml::engine::{Engine, is_true_ci, is_value};
use crate::plantuml::model::{ElemRef, element_by_id};

use crate::plantuml::{ICON_SCALE, PLANTUML_SHADOW_PROPERTY, is_supported_icon};

use super::METADATA_FONT_SIZE_RATIO;

/// `view.getAutomaticLayout()` for the view kinds that carry content
/// (`ImageView`/`FilteredView` do not). The core `View::content()`
/// accessor borrows from `&self`, so the port matches the variants to
/// keep the view's `'a`.
pub(crate) fn automatic_layout(view: View<'_>) -> Option<&AutomaticLayout> {
    match view {
        View::Custom(v) => v.content.automatic_layout.as_ref(),
        View::Landscape(v) => v.content.automatic_layout.as_ref(),
        View::Context(v) => v.content.automatic_layout.as_ref(),
        View::Container(v) => v.content.automatic_layout.as_ref(),
        View::Component(v) => v.content.automatic_layout.as_ref(),
        View::Dynamic(v) => v.content.automatic_layout.as_ref(),
        View::Deployment(v) => v.content.automatic_layout.as_ref(),
        View::Filtered(_) | View::Image(_) => None,
    }
}

/// `calculateMetadataFontSize`: `(int)Math.floor(fontSize * 0.7)`.
#[allow(clippy::cast_possible_truncation)] // the Java `(int)` truncation IS the contract
pub(super) fn metadata_font_size(font_size: i32) -> i32 {
    (f64::from(font_size) * METADATA_FONT_SIZE_RATIO).floor() as i32
}

/// The scheme's background/color defaults (`Styles.DEFAULT_*`).
pub(crate) fn defaults(scheme: ColorScheme) -> (&'static str, &'static str) {
    match scheme {
        ColorScheme::Dark => (DEFAULT_BACKGROUND_DARK, DEFAULT_COLOR_DARK),
        ColorScheme::Light => (DEFAULT_BACKGROUND_LIGHT, DEFAULT_COLOR_LIGHT),
    }
}

/// The color/`null` formatter: `%s` on a null string renders the
/// literal `null`.
pub(crate) fn color_or_null(value: Option<&str>) -> &str {
    value.unwrap_or("null")
}

/// The first non-empty string across the official three-source chain.
pub(super) fn field_str<'a>(
    first: Option<&'a ElementStyle>,
    second: Option<&'a ElementStyle>,
    third: Option<&'a ElementStyle>,
    get: fn(&'a ElementStyle) -> Option<&'a str>,
) -> Option<String> {
    for source in [first, second, third].into_iter().flatten() {
        if let Some(value) = get(source)
            && is_value(value)
        {
            return Some(value.to_string());
        }
    }
    None
}

/// The first non-null `Integer` across the chain.
pub(super) fn field_i32(
    first: Option<&ElementStyle>,
    second: Option<&ElementStyle>,
    third: Option<&ElementStyle>,
    get: fn(&ElementStyle) -> Option<i32>,
) -> Option<i32> {
    [first, second, third].into_iter().flatten().find_map(get)
}

/// The first non-null border across the chain.
pub(super) fn field_border(
    first: Option<&ElementStyle>,
    second: Option<&ElementStyle>,
    third: Option<&ElementStyle>,
) -> Option<LineStyle> {
    [first, second, third]
        .into_iter()
        .flatten()
        .find_map(|source| source.border)
}

/// The first `isSupportedIcon` icon across the chain.
pub(super) fn field_icon(
    first: Option<&ElementStyle>,
    second: Option<&ElementStyle>,
    third: Option<&ElementStyle>,
) -> Option<String> {
    [first, second, third]
        .into_iter()
        .flatten()
        .find(|source| is_supported_icon(source.icon.as_deref()))
        .and_then(|source| source.icon.clone())
}

/// The `Border`/`LineStyle` → `PlantUML` dash mapping shared by the five
/// style constructors: `Dotted` → `w-w`, `Dashed` → `5w-5w`,
/// `Solid`/absent → `0`.
pub(super) fn line_style(border: Option<LineStyle>, width: i32) -> String {
    match border {
        Some(LineStyle::Dotted) => format!("{width}-{width}"),
        Some(LineStyle::Dashed) => {
            let dash = width * 5;
            format!("{dash}-{dash}")
        }
        Some(LineStyle::Solid) | None => "0".to_string(),
    }
}

/// The shadow gate: `"true"` (case-insensitive) from the property
/// `"plantuml.shadow"`, default `"false"` — the MERGED styles only (the synthesized
/// boundary/group styles are always false).
pub(super) fn shadow_of(style: &ElementStyle) -> bool {
    style
        .properties
        .get(PLANTUML_SHADOW_PROPERTY)
        .is_some_and(is_true_ci)
}

/// The legend description: `name`, minus the `Element,` / `Relationship,`
/// prefix, with `replaceAll(",", ", ")`.
pub(super) fn legend_description(name: &str) -> String {
    let mut description = name;
    for prefix in ["Element,", "Relationship,"] {
        if let Some(rest) = description.strip_prefix(prefix) {
            description = rest;
            break;
        }
    }
    description.replace(',', ", ")
}

/// The supported-icon `\n\n<img:URL{scale=0.5}>` leg.
pub(super) fn icon_leg(icon: Option<&str>) -> String {
    if is_supported_icon(icon) {
        format!(
            "\\n\\n<img:{}{{scale={ICON_SCALE}}}>",
            icon.unwrap_or_default()
        )
    } else {
        String::new()
    }
}

/// The view's per-kind `getName()` (the header's title fallback). Only
/// Context/Container/Component scopes crash when unresolved (→
/// [`ExportError::ReferenceAbort`]); a dangling Dynamic scope is the null
/// `element` field ("Dynamic View") and a dangling Deployment scope the
/// null `softwareSystem` (environment-only name).
pub(crate) fn view_name(e: &Engine, view: View<'_>) -> Result<String, ExportError> {
    let scoped_system = |id: &Id| -> Result<String, ExportError> {
        element_by_id(e.model(), id)
            .ok_or(ExportError::ReferenceAbort(
                "view scope does not resolve (Java getSoftwareSystem() NPE)",
            ))
            .map(|element| element.name().to_string())
    };
    Ok(match view {
        // `getTitle()` on the fallback path is null/empty — `null` renders
        // the literal.
        View::Custom(v) => {
            format!("Custom View: {}", v.base.title.as_deref().unwrap_or("null"))
        }
        View::Landscape(_) => "System Landscape View".to_string(),
        View::Context(v) => format!(
            "System Context View: {}",
            scoped_system(&v.software_system_id)?
        ),
        View::Container(v) => format!("Container View: {}", scoped_system(&v.software_system_id)?),
        View::Component(v) => {
            let container = element_by_id(e.model(), &v.container_id).ok_or(
                ExportError::ReferenceAbort("component view container does not resolve (NPE)"),
            )?;
            let system = container
                .parent_id()
                .and_then(|pid| element_by_id(e.model(), pid))
                .ok_or(ExportError::ReferenceAbort(
                    "component view container has no system (Java NPE)",
                ))?;
            format!("Component View: {} - {}", system.name(), container.name())
        }
        View::Dynamic(v) => match v
            .element_id
            .as_ref()
            .and_then(|id| element_by_id(e.model(), id))
        {
            None => "Dynamic View".to_string(),
            Some(element) if element.kind() == ElementKind::Container => {
                let parent = element
                    .parent_id()
                    .and_then(|pid| element_by_id(e.model(), pid))
                    .ok_or(ExportError::ReferenceAbort(
                        "dynamic view scope container has no parent (Java NPE)",
                    ))?;
                format!("Dynamic View: {} - {}", parent.name(), element.name())
            }
            Some(element) => format!("Dynamic View: {}", display_name(e, element)?),
        },
        View::Deployment(v) => match v
            .software_system_id
            .as_ref()
            .and_then(|id| element_by_id(e.model(), id))
        {
            Some(system) => format!("Deployment View: {} - {}", system.name(), v.environment),
            None => format!("Deployment View: {}", v.environment),
        },
        // The engine never exports these kinds through the PlantUML body.
        View::Filtered(_) | View::Image(_) => {
            return Err(ExportError::ReferenceAbort(
                "filtered/image views have no reference getName()",
            ));
        }
    })
}

/// The display name — an instance's name is its target's (the official
/// delegation; a dangling
/// target crashes).
pub(crate) fn display_name(e: &Engine, element: ElemRef<'_>) -> Result<String, ExportError> {
    if matches!(
        element.kind(),
        ElementKind::SoftwareSystemInstance | ElementKind::ContainerInstance
    ) {
        Ok(element
            .target(e.model())
            .ok_or(ExportError::ReferenceAbort(
                "instance references a missing element (Java getElement().getName() NPE)",
            ))?
            .name()
            .to_string())
    } else {
        Ok(element.name().to_string())
    }
}

/// The MERGED element style — `Styles.findElementStyle(Element,
/// ColorScheme)`: instances PREPEND the parent's tags.
pub(crate) fn element_style_of(
    e: &Engine,
    element: ElemRef<'_>,
) -> Result<ElementStyle, ExportError> {
    let styles = &e.configuration().styles;
    let scheme = e.color_scheme();
    let tags = if matches!(
        element.kind(),
        ElementKind::SoftwareSystemInstance | ElementKind::ContainerInstance
    ) {
        let target = element
            .target(e.model())
            .ok_or(ExportError::ReferenceAbort(
                "instance references a missing element (Java getTags() NPE)",
            ))?;
        instance_tag_chain(&target.tags_csv(), &element.tags_csv())
    } else {
        element.tags_csv()
    };
    Ok(styles.element_style(&tags, scheme))
}

/// `AbstractPlantUMLExporter.idOf` — the parent chain joined by `.`; a
/// top-level deployment node is `filter(environment).filter(name)`;
/// instances are `filter(targetName)_instanceId`; custom elements carry
/// the raw model id.
pub(crate) fn id_of(e: &Engine, element: ElemRef<'_>) -> Result<String, ExportError> {
    let leaf = id_leaf(e, element)?;
    Ok(match element.parent_id() {
        Some(pid) => {
            let parent = element_by_id(e.model(), pid).ok_or(ExportError::ReferenceAbort(
                "element parent does not resolve (Java getParent() NPE)",
            ))?;
            format!("{}.{}", id_of(e, parent)?, leaf)
        }
        None => {
            if element.kind() == ElementKind::DeploymentNode {
                format!("{}.{}", filter(element.environment().unwrap_or("")), leaf)
            } else {
                leaf
            }
        }
    })
}

/// `AbstractPlantUMLExporter.id(ModelItem)` per kind.
pub(super) fn id_leaf(e: &Engine, element: ElemRef<'_>) -> Result<String, ExportError> {
    Ok(match element.kind() {
        ElementKind::CustomElement => element.id().to_string(),
        ElementKind::SoftwareSystemInstance | ElementKind::ContainerInstance => {
            let target = element
                .target(e.model())
                .ok_or(ExportError::ReferenceAbort(
                    "instance references a missing element (Java getName() NPE)",
                ))?;
            format!(
                "{}_{}",
                filter(target.name()),
                element
                    .instance_id()
                    .map_or_else(|| "null".to_string(), |id| id.to_string())
            )
        }
        _ => filter(element.name()),
    })
}

/// `idOf(getRelationship().getSource()/getDestination())` — a dangling
/// endpoint is the official null-pointer crash.
pub(crate) fn element_ref_of(e: &Engine, id: &Id) -> Result<String, ExportError> {
    let element = element_by_id(e.model(), id).ok_or(ExportError::ReferenceAbort(
        "relationship endpoint does not resolve (Java getSource()/getDestination() NPE)",
    ))?;
    id_of(e, element)
}

/// Filter: drop every non-word
/// character (Unicode letters/digits/underscore survive).
pub(crate) fn filter(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// The split with limit 0: trailing empty pieces are dropped.
pub(crate) fn java_split(s: &str, sep: char) -> Vec<&str> {
    let mut parts: Vec<&str> = s.split(sep).collect();
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

/// Standard Base64 (standard alphabet, `=` padding) over the
/// UTF-8 bytes — the class-selector hash.
pub(super) fn b64(input: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let mut b = [0u8; 3];
        b[..chunk.len()].copy_from_slice(chunk);
        let n = u32::from_be_bytes([0, b[0], b[1], b[2]]);
        // one index shape for all four sextets — the old mix
        // of `usize::try_from(…).unwrap_or(0)` and `as usize` was
        // inconsistent; each value is masked to its 6 bits before indexing.
        out.push(char::from(TABLE[((n >> 18) & 63) as usize]));
        out.push(char::from(TABLE[((n >> 12) & 63) as usize]));
        out.push(if chunk.len() > 1 {
            char::from(TABLE[((n >> 6) & 63) as usize])
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            char::from(TABLE[(n & 63) as usize])
        } else {
            '='
        });
    }
    out
}
