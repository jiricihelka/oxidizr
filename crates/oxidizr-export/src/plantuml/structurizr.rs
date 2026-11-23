//! The Structurizr-PlantUML dialect — the official exporter port,
//! including the seven `PlantUML*Style` renderers.
//!
//! Every line format, gate and quirk mirrors the official exporter: the
//! `<style></style>` placeholder replaced by the seven name-sorted class
//! passes, the base64 class selectors, the boundary/group/DN rectangles,
//! the per-kind title fallback (missing names crash — the official null paths
//! surface as [`ExportError::ReferenceAbort`]) and the legend built from
//! the LAST body export's style set.

use oxidizr_core::Id;
use oxidizr_core::styles::{
    DEFAULT_BACKGROUND_DARK, DEFAULT_BACKGROUND_LIGHT, DEFAULT_COLOR_DARK, DEFAULT_COLOR_LIGHT,
    instance_tag_chain,
};
use oxidizr_core::validate::ElementKind;
use oxidizr_core::views::{
    AutomaticLayout, ColorScheme, ElementStyle, LineStyle, RankDirection, RelationshipView, Shape,
    View,
};

use crate::diagram::{Diagram, Legend};
use crate::error::ExportError;
use crate::indent::IndentingWriter;
use crate::plantuml::engine::{Dialect, Engine, is_null_or_empty, is_true_ci, is_value};
use crate::plantuml::model::{ElemRef, element_by_id};
use crate::plantuml::{
    DIAGRAM_DESCRIPTION_TAG, DIAGRAM_TITLE_TAG, ICON_SCALE, PLANTUML_ANIMATION_PROPERTY,
    PLANTUML_BOUNDARIES_PROPERTY, PLANTUML_INCLUDES_PROPERTY, PLANTUML_SEQUENCE_DIAGRAM_PROPERTY,
    PLANTUML_SHADOW_PROPERTY, PLANTUML_SKINPARAMS_PROPERTY, PLANTUML_TITLE_PROPERTY,
    is_supported_icon, java_cmp_str, plantuml_sequence_shape_of, plantuml_shape_of,
};

/// `AbstractPlantUMLExporter.DEFAULT_FONT_SIZE`.
const DEFAULT_FONT_SIZE: i32 = 24;
/// `StructurizrPlantUMLExporter.DEFAULT_WIDTH`.
const DEFAULT_WIDTH: i32 = 450;
/// `StructurizrPlantUMLExporter.DEFAULT_STROKE_WIDTH`.
const DEFAULT_STROKE_WIDTH: i32 = 2;
/// `PlantUMLStyle.SHADOW_DISTANCE`.
const SHADOW_DISTANCE: i32 = 10;
/// `METADATA_FONT_SIZE_RATIO`.
const METADATA_FONT_SIZE_RATIO: f64 = 0.7;
/// The relationship-style width fallback (the official merged style
/// defaults it to 200 — same fallback as the base style builder).
const DEFAULT_REL_WIDTH: i32 = 200;

/// The dialect: the per-body-export style set (`HashSet` semantics — first
/// add wins by class + selector), the exporter-scoped group counter
/// (`groupId` — NEVER reset between views/exports) and the style-set
/// serialization order.
#[derive(Debug, Default)]
pub struct StructurizrDialect {
    styles: Vec<PlantUMLStyle>,
    group_id: i32,
}

impl StructurizrDialect {
    /// A fresh dialect (empty style set; group ids start at 1).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Style-set add — `HashSet` semantics: a style equal to an
    /// existing entry DROPS the new one (first wins). Equality is class +
    /// class selector, except the root (class only) and the legend (its
    /// selector is constant, so effectively class only).
    fn add_style(&mut self, style: PlantUMLStyle) {
        // the new style's tag/selector/class-only test are loop
        // invariants — computed once, not per stored-style comparison
        // (`selector()` re-runs the b64 encode on every call).
        let target_tag = style.class_tag();
        let target_selector = style.selector();
        let dup = self.styles.iter().any(|s| {
            s.class_tag() == target_tag
                && (matches!(style, PlantUMLStyle::Root { .. } | PlantUMLStyle::Legend)
                    || s.selector() == target_selector)
        });
        if !dup {
            self.styles.push(style);
        }
    }

    /// `writeStyles`: one stable name sort (`Comparator.comparing(getName)`
    /// — `java_cmp_str`; the `HashSet` iteration order is unobservable in
    /// the official tooling, so stable insertion order is this port's
    /// deterministic choice), then the seven class passes, then the
    /// `<style></style>` placeholder replacement.
    fn write_styles(&self, w: &mut IndentingWriter) {
        let mut order: Vec<&PlantUMLStyle> = self.styles.iter().collect();
        order.sort_by(|a, b| java_cmp_str(a.name(), b.name()));
        let mut all = String::new();
        for class_rank in 0..7u8 {
            for style in &order {
                if style.class_rank() == class_rank {
                    all.push_str(&style.render());
                }
            }
        }
        w.replace("<style></style>", &format!("<style>\n{all}</style>"));
    }

    /// The skin-params write — `Result` because the second split part is
    /// index-out-of-range crash when an entry's value splits
    /// empty (`"foo="` — the official split drops the trailing empty).
    pub(crate) fn write_skin_params<'a>(
        e: &Engine<'a>,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let comma_separated = e.property(view.base(), PLANTUML_SKINPARAMS_PROPERTY, "");
        if is_value(comma_separated) {
            w.write_line(None);
            w.write_line(Some("skinparam {"));
            w.indent();
            for entry in java_split(comma_separated, ',') {
                if !is_value(entry) || !entry.contains('=') {
                    continue;
                }
                let entry = entry.trim();
                let parts = java_split(entry, '=');
                let Some(value) = parts.get(1) else {
                    return Err(ExportError::ReferenceAbort(
                        "skinparam with an empty value (Java split(\"=\")[1] AIOOBE)",
                    ));
                };
                w.write_line(Some(&format!("{} {}", parts[0], value)));
            }
            w.outdent();
            w.write_line(Some("}"));
        }
        w.write_line(None);
        Ok(())
    }

    /// `writeIncludes`.
    pub(crate) fn write_includes<'a>(e: &Engine<'a>, view: View<'a>, w: &mut IndentingWriter) {
        let comma_separated = e.property(view.base(), PLANTUML_INCLUDES_PROPERTY, "");
        if is_value(comma_separated) {
            for include in java_split(comma_separated, ',') {
                if is_value(include) {
                    w.write_line(Some(&format!("!include {}", include.trim())));
                }
            }
            w.write_line(None);
        }
    }

    /// `includeTitle` — the one case-SENSITIVE property gate
    /// (`"true".equals`).
    pub(crate) fn include_title<'a>(e: &Engine<'a>, view: View<'a>) -> bool {
        e.property(view.base(), PLANTUML_TITLE_PROPERTY, "true") == "true"
    }

    /// The base-header title font: `titleStyle != null ? getFontSize() :
    /// DEFAULT_FONT_SIZE` — the `Integer` `null` renders the literal
    /// `null` through `%s`.
    pub(crate) fn font_of(title_style: Option<&ElementStyle>) -> String {
        title_style.map_or_else(
            || DEFAULT_FONT_SIZE.to_string(),
            |style| {
                style
                    .font_size
                    .map_or_else(|| "null".to_string(), |f| f.to_string())
            },
        )
    }

    /// `findGroupStyle` — the synthesized `ElementStyle("")` shape (its
    /// empty property map is why boundary/group shadows are ALWAYS false)
    /// with the official line-711 bug mirrored: the all-groups icon
    /// fallback assigns the all-groups COLOR.
    fn find_group_style(e: &Engine, group: &str) -> ElementSpec {
        let styles = &e.configuration().styles;
        let scheme = e.color_scheme();
        let (default_bg, default_color) = defaults(scheme);
        let group_style = styles.find_element_style(&format!("Group:{group}"), scheme);
        let all_style = styles.find_element_style("Group", scheme);
        let (g, a) = (group_style.as_ref(), all_style.as_ref());
        // THE 711 BUG: `icon = elementStyleForAllGroups.getColor();`.
        let icon = match (g, a) {
            (Some(g), _) if is_supported_icon(g.icon.as_deref()) => g.icon.clone(),
            (_, Some(a)) if is_supported_icon(a.icon.as_deref()) => a.color.clone(),
            _ => None,
        };
        ElementSpec {
            background: field_str(g, a, None, |s| s.background.as_deref())
                .or(Some(default_bg.to_string())),
            stroke: field_str(g, a, None, |s| s.stroke.as_deref())
                .or(Some(default_color.to_string())),
            stroke_width: field_i32(g, a, None, |s| s.stroke_width).unwrap_or(DEFAULT_STROKE_WIDTH),
            border: field_border(g, a, None).or(Some(LineStyle::Dotted)),
            color: field_str(g, a, None, |s| s.color.as_deref())
                .or(Some(default_color.to_string())),
            icon,
            font_size: field_i32(g, a, None, |s| s.font_size).unwrap_or(DEFAULT_FONT_SIZE),
        }
    }

    /// `findBoundaryStyle` — same synthesized shape; the chain is
    /// `Boundary:<SoftwareSystem|Container>`, `Boundary`, then the
    /// element's MERGED style (background excepted — its chain stops at
    /// the all-boundaries style).
    fn find_boundary_style(e: &Engine, element: ElemRef) -> ElementSpec {
        let styles = &e.configuration().styles;
        let scheme = e.color_scheme();
        let (default_bg, default_color) = defaults(scheme);
        let type_name = if element.kind() == ElementKind::SoftwareSystem {
            "SoftwareSystem"
        } else {
            "Container"
        };
        let boundary = styles.find_element_style(&format!("Boundary:{type_name}"), scheme);
        let all = styles.find_element_style("Boundary", scheme);
        let merged = e
            .configuration()
            .styles
            .element_style(&element.tags_csv(), e.color_scheme());
        let (b, a, m) = (boundary.as_ref(), all.as_ref(), Some(&merged));
        ElementSpec {
            background: field_str(b, a, None, |s| s.background.as_deref())
                .or(Some(default_bg.to_string())),
            stroke: field_str(b, a, m, |s| s.stroke.as_deref()).or(Some(default_color.to_string())),
            stroke_width: field_i32(b, a, m, |s| s.stroke_width).unwrap_or(DEFAULT_STROKE_WIDTH),
            border: field_border(b, a, m).or(Some(LineStyle::Dotted)),
            color: field_str(b, a, m, |s| s.color.as_deref()).or(Some(default_color.to_string())),
            icon: field_icon(b, a, m),
            font_size: field_i32(b, a, m, |s| s.font_size).unwrap_or(DEFAULT_FONT_SIZE),
        }
    }

    /// The group-boundary display name: the text after the LAST separator;
    /// when the separator is absent the official code still cuts at the
    /// separator length minus one, and an out-of-range cut aborts the
    /// export (a pinned index-out-of-range).
    pub(crate) fn group_display_name(
        group: &str,
        separator: Option<&str>,
    ) -> Result<String, ExportError> {
        let Some(separator) = separator.filter(|s| is_value(s)) else {
            return Ok(group.to_string());
        };
        // `separator.chars().count() >= 1` (the `is_value` filter), so the
        // absent-separator arm cannot underflow.
        let units: Vec<char> = group.chars().collect();
        let start: usize = match group.rfind(separator) {
            Some(index) => group[..index].chars().count() + separator.chars().count(),
            None => separator.chars().count() - 1,
        };
        if start > units.len() {
            return Err(ExportError::ReferenceAbort(
                "group separator cut outside the group name (Java substring)",
            ));
        }
        Ok(units[start..].iter().collect())
    }
}

impl Dialect for StructurizrDialect {
    fn write_header<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        // The style set resets to an empty `HashSet` per body export.
        self.styles.clear();

        // `AbstractPlantUMLExporter.writeHeader`.
        w.write_line(Some("@startuml"));
        if Self::include_title(e, view) {
            let styles = &e.configuration().styles;
            let title_style = styles.find_element_style(DIAGRAM_TITLE_TAG, e.color_scheme());
            let description_style =
                styles.find_element_style(DIAGRAM_DESCRIPTION_TAG, e.color_scheme());
            let title = match view.base().title.as_deref() {
                Some(title) if is_value(title) => title.to_string(),
                // The view-name fallback — the official tooling crashes on a
                // dangling Context/Container/Component scope.
                _ => view_name(e, view)?,
            };
            let description = view.base().description.as_deref().filter(|d| is_value(d));
            match description {
                None => w.write_line(Some(&format!(
                    "title <size:{}>{}</size>",
                    Self::font_of(title_style.as_ref()),
                    title
                ))),
                Some(description) => w.write_line(Some(&format!(
                    "title <size:{}>{}</size>\\n<size:{}>{}</size>",
                    Self::font_of(title_style.as_ref()),
                    title,
                    Self::font_of(description_style.as_ref()),
                    description
                ))),
            }
        }
        w.write_line(None);
        w.write_line(Some("set separator none"));

        if self.render_as_sequence_diagram(e, view) {
            if is_true_ci(e.property(view.base(), PLANTUML_BOUNDARIES_PROPERTY, "true")) {
                w.write_line(Some("!pragma teoz true"));
            }
        } else if let Some(layout) = automatic_layout(view) {
            // Only `LeftRight` differs from the `top to bottom` default
            // (the null rank direction falls into the default arm).
            w.write_line(Some(match layout.rank_direction {
                Some(RankDirection::LeftRight) => "left to right direction",
                _ => "top to bottom direction",
            }));
            // The rank separations divide by (300/60) and (300/30) —
            // integer division (truncating in both languages).
            w.write_line(Some(&format!(
                "skinparam ranksep {}",
                layout.rank_separation / 5
            )));
            w.write_line(Some(&format!(
                "skinparam nodesep {}",
                layout.node_separation / 10
            )));
        } else {
            w.write_line(Some("top to bottom direction"));
        }

        w.write_line(Some("hide stereotype"));
        Self::write_skin_params(e, view, w)?;
        w.write_line(Some("<style></style>"));
        w.write_line(None);

        // The root style (the official font-name local stays null).
        let (background, color) = match e.color_scheme() {
            ColorScheme::Dark => (DEFAULT_BACKGROUND_DARK, DEFAULT_COLOR_DARK),
            ColorScheme::Light => (DEFAULT_BACKGROUND_LIGHT, DEFAULT_COLOR_LIGHT),
        };
        self.add_style(PlantUMLStyle::Root {
            background: background.to_string(),
            color: color.to_string(),
        });

        Self::write_includes(e, view, w);
        Ok(())
    }

    fn write_footer<'a>(&mut self, _e: &Engine<'a>, _view: View<'a>, w: &mut IndentingWriter) {
        w.write_line(Some("@enduml"));
        self.write_styles(w);
    }

    fn start_group_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        group: &str,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let group_name = Self::group_display_name(
            group,
            e.model().properties.get("structurizr.groupSeparator"),
        )?;
        let spec = Self::find_group_style(e, group);
        self.add_style(PlantUMLStyle::Group {
            name: group.to_string(),
            background: spec.background,
            color: spec.color,
            stroke: spec.stroke,
            stroke_width: spec.stroke_width,
            line_style: line_style(spec.border, spec.stroke_width),
            font_size: spec.font_size,
            shadow: false, // the synthesized style carries no properties
        });
        if !self.render_as_sequence_diagram(e, view) {
            // The group icon gate is NON-EMPTY (trimmed check), NOT
            // `isSupportedIcon` — the 711-bug color value renders too.
            let icon = spec
                .icon
                .as_deref()
                .filter(|icon| is_value(icon))
                .map_or_else(String::new, |icon| {
                    format!("\\n\\n<img:{icon}{{scale={ICON_SCALE}}}>")
                });
            self.group_id += 1;
            w.write_line(Some(&format!(
                "rectangle \"{group_name}{icon}\" <<Group-{}>> as group{} {{",
                b64(group),
                self.group_id
            )));
            w.indent();
        }
        Ok(())
    }

    fn end_group_boundary<'a>(&mut self, e: &Engine<'a>, view: View<'a>, w: &mut IndentingWriter) {
        if !self.render_as_sequence_diagram(e, view) {
            w.outdent();
            w.write_line(Some("}"));
            w.write_line(None);
        }
    }

    fn start_software_system_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        system: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) {
        if !self.render_as_sequence_diagram(e, view) {
            let spec = Self::find_boundary_style(e, system);
            self.add_style(PlantUMLStyle::Boundary {
                name: system.name().to_string(),
                background: spec.background,
                color: spec.color,
                stroke: spec.stroke,
                stroke_width: spec.stroke_width,
                line_style: line_style(spec.border, spec.stroke_width),
                font_size: spec.font_size,
                shadow: false, // synthesized style: no properties
            });
            w.write_line(Some(&format!(
                "rectangle \"{}\\n<size:{}>{}</size>\" <<Boundary-{}>> {{",
                system.name(),
                metadata_font_size(spec.font_size),
                e.type_of(system, true),
                b64(system.name())
            )));
            w.indent();
        }
    }

    fn end_software_system_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) {
        if !self.render_as_sequence_diagram(e, view) {
            w.outdent();
            w.write_line(Some("}"));
            w.write_line(None);
        }
    }

    fn start_container_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        container: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) {
        if !self.render_as_sequence_diagram(e, view) {
            let spec = Self::find_boundary_style(e, container);
            self.add_style(PlantUMLStyle::Boundary {
                name: container.name().to_string(),
                background: spec.background,
                color: spec.color,
                stroke: spec.stroke,
                stroke_width: spec.stroke_width,
                line_style: line_style(spec.border, spec.stroke_width),
                font_size: spec.font_size,
                shadow: false, // synthesized style: no properties
            });
            w.write_line(Some(&format!(
                "rectangle \"{}\\n<size:{}>{}</size>\" <<Boundary-{}>> {{",
                container.name(),
                metadata_font_size(spec.font_size),
                e.type_of(container, true),
                b64(container.name())
            )));
            w.indent();
        }
    }

    fn end_container_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) {
        if !self.render_as_sequence_diagram(e, view) {
            w.outdent();
            w.write_line(Some("}"));
            w.write_line(None);
        }
    }

    fn start_deployment_node_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        node: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        // The MERGED element style (shadow is real here).
        let style = element_style_of(e, node)?;
        let stroke_width = style.stroke_width.unwrap_or(DEFAULT_STROKE_WIDTH);
        let font_size = style.font_size.unwrap_or(DEFAULT_FONT_SIZE);
        let tag = style.tag.clone();
        let icon_url = style.icon.clone();
        self.add_style(PlantUMLStyle::DeploymentNode {
            name: tag.clone(),
            background: style.background.clone(),
            color: style.color.clone(),
            stroke: style.stroke.clone(),
            stroke_width,
            line_style: line_style(style.border, stroke_width),
            font_size,
            icon: icon_url.clone(),
            shadow: shadow_of(&style),
            width: None, // legend-only (`MaximumWidth` written when Some)
        });
        let icon = if is_supported_icon(icon_url.as_deref()) {
            format!(
                "\\n\\n<img:{}{{scale={ICON_SCALE}}}>",
                icon_url.unwrap_or_default()
            )
        } else {
            String::new()
        };
        let url = node
            .url()
            .filter(|u| is_value(u))
            .map_or_else(String::new, |u| format!(" [[{u}]]"));
        // `!"1".equals(getInstances())` — the instance-count suffix (the
        // field defaults to "1"; a `null` renders the literal `null`).
        let instances = node.instances().unwrap_or("1");
        let name = if instances == "1" {
            node.name().to_string()
        } else {
            format!("{} (x{instances})", node.name())
        };
        let id = id_of(e, node)?;
        w.write_line(Some(&format!(
            "rectangle \"{name}\\n<size:{}>{}</size>{icon}\" <<DeploymentNode-{}>> as {id}{url} {{",
            metadata_font_size(font_size),
            e.type_of(node, true),
            b64(&tag)
        )));
        w.indent();
        if !e.is_visible(view, node)? {
            w.write_line(Some(&format!("hide {id}")));
        }
        Ok(())
    }

    fn end_deployment_node_boundary<'a>(
        &mut self,
        _e: &Engine<'a>,
        _view: View<'a>,
        w: &mut IndentingWriter,
    ) {
        w.outdent();
        w.write_line(Some("}"));
        w.write_line(None);
    }

    fn start_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        _view: View<'a>,
        element: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) {
        let spec = Self::find_boundary_style(e, element);
        self.add_style(PlantUMLStyle::Boundary {
            name: element.name().to_string(),
            background: spec.background,
            color: spec.color,
            stroke: spec.stroke,
            // The sequence box passes `DEFAULT_STROKE_WIDTH` outright.
            stroke_width: DEFAULT_STROKE_WIDTH,
            line_style: line_style(spec.border, DEFAULT_STROKE_WIDTH),
            font_size: spec.font_size,
            shadow: false, // synthesized style: no properties
        });
        w.write_line(Some(&format!(
            "box \"{}\\n<size:{}>{}</size>\" <<Boundary-{}>>",
            element.name(),
            metadata_font_size(spec.font_size),
            e.type_of(element, true),
            b64(element.name())
        )));
        w.indent();
    }

    fn end_boundary<'a>(&mut self, _e: &Engine<'a>, _view: View<'a>, w: &mut IndentingWriter) {
        w.outdent();
        w.write_line(Some("end box"));
    }

    fn write_element<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        element: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let mut style = element_style_of(e, element)?;
        let width = style.width.unwrap_or(DEFAULT_WIDTH);
        let sequence = self.render_as_sequence_diagram(e, view);
        let sequence_shape = plantuml_sequence_shape_of(style.shape);
        if sequence && (sequence_shape == "actor" || sequence_shape == "database") {
            // The label-sits-outside shapes take the stroke as their color.
            style.color.clone_from(&style.stroke);
        }
        let stroke_width = if sequence {
            DEFAULT_STROKE_WIDTH
        } else {
            style.stroke_width.unwrap_or(DEFAULT_STROKE_WIDTH)
        };
        let font_size = style.font_size.unwrap_or(DEFAULT_FONT_SIZE);
        let tag = style.tag.clone();
        let icon_url = style.icon.clone();
        self.add_style(PlantUMLStyle::Element {
            name: tag.clone(),
            shape: style.shape,
            width,
            background: style.background.clone(),
            color: style.color.clone(),
            stroke: style.stroke.clone(),
            stroke_width,
            line_style: line_style(style.border, stroke_width),
            font_size,
            icon: icon_url.clone(),
            shadow: shadow_of(&style),
        });
        let selector = format!("Element-{}", b64(&tag));
        let metadata_font = metadata_font_size(font_size);

        if sequence {
            w.write_line(Some(&format!(
                "{sequence_shape} \"{}\\n<size:{metadata_font}>{}</size>\" as {} <<{selector}>>",
                display_name(e, element)?,
                e.type_of(element, true),
                id_of(e, element)?
            )));
        } else {
            let mut shape = plantuml_shape_of(style.shape);
            let mut name = element.name();
            let mut description = element.description();
            let mut type_of = e.type_of(element, true);
            let mut url = element.url().map(str::to_string);
            if matches!(
                element.kind(),
                ElementKind::SoftwareSystemInstance | ElementKind::ContainerInstance
            ) {
                let target = element
                    .target(e.model())
                    .ok_or(ExportError::ReferenceAbort(
                        "instance references a missing element (Java getElement() NPE)",
                    ))?;
                name = target.name();
                description = target.description();
                type_of = e.type_of(target, true);
                shape = plantuml_shape_of(
                    e.configuration()
                        .styles
                        .element_style(&target.tags_csv(), e.color_scheme())
                        .shape,
                );
                if url.as_deref().is_none_or(|u| !is_value(u)) {
                    url = target.url().map(str::to_string);
                }
            }
            let url = url
                .as_deref()
                .filter(|u| is_value(u))
                .map_or_else(String::new, |u| format!(" [[{u}]]"));
            let description = if is_null_or_empty(description) || style.description == Some(false) {
                String::new()
            } else {
                format!("\\n\\n{}", description.unwrap_or_default())
            };
            let type_leg = if !is_value(&type_of) || style.metadata == Some(false) {
                String::new()
            } else {
                format!("\\n<size:{metadata_font}>{type_of}</size>")
            };
            let icon = if is_supported_icon(icon_url.as_deref()) {
                format!(
                    "\\n\\n<img:{}{{scale={ICON_SCALE}}}>",
                    icon_url.unwrap_or_default()
                )
            } else {
                String::new()
            };
            let id = id_of(e, element)?;
            w.write_line(Some(&format!(
                "{shape} \"=={name}{type_leg}{icon}{description}\" <<{selector}>> as {id}{url}"
            )));
            if !e.is_visible(view, element)? {
                w.write_line(Some(&format!("hide {id}")));
            }
        }
        Ok(())
    }

    fn write_relationship<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        relationship_view: &'a RelationshipView,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let relationship = e.relationship(relationship_view)?;
        let style =
            e.configuration()
                .styles
                .relationship_style(relationship, e.model(), e.color_scheme());
        let sequence = self.render_as_sequence_diagram(e, view);
        let thickness = if sequence {
            DEFAULT_STROKE_WIDTH
        } else {
            style.thickness.unwrap_or(DEFAULT_STROKE_WIDTH)
        };
        let font_size = style.font_size.unwrap_or(DEFAULT_FONT_SIZE);
        let tag = style.tag.clone();
        self.add_style(PlantUMLStyle::Relationship {
            name: tag.clone(),
            color: style.color.clone(),
            line_style: line_style(style.style, thickness),
            thickness,
            font_size,
            width: style.width.unwrap_or(DEFAULT_REL_WIDTH),
        });
        let metadata_font = metadata_font_size(font_size);
        let mut description = String::new();
        if let Some(order) = relationship_view.order.as_deref() {
            if is_value(order) {
                description = format!("{order}: ");
            }
        }
        description.push_str(
            relationship_view
                .description
                .as_deref()
                .filter(|d| is_value(d))
                .or_else(|| relationship.description.as_deref().filter(|d| is_value(d)))
                .unwrap_or_default(),
        );
        let technology = relationship.technology.as_deref().unwrap_or("");
        let technology = if is_value(technology) {
            format!("\\n<size:{metadata_font}>[{technology}]</size>")
        } else {
            String::new()
        };
        let response = relationship_view.response == Some(true);
        let source = element_ref_of(e, &relationship.source)?;
        let destination = element_ref_of(e, &relationship.destination)?;
        if sequence {
            let arrow = if response { "<--" } else { "->" };
            w.write_line(Some(&format!(
                "{source} {arrow} {destination} <<Relationship-{}>> : {description}{technology}",
                b64(&tag)
            )));
        } else {
            let arrow = if response { "<--" } else { "-->" };
            w.write_line(Some(&format!(
                "{source} {arrow} {destination} <<Relationship-{}>> : \"{description}{technology}\"",
                b64(&tag)
            )));
        }
        Ok(())
    }

    fn is_animation_supported<'a>(&self, e: &Engine<'a>, view: View<'a>) -> bool {
        is_true_ci(e.property(view.base(), PLANTUML_ANIMATION_PROPERTY, "false"))
    }

    fn render_as_sequence_diagram<'a>(&self, e: &Engine<'a>, view: View<'a>) -> bool {
        matches!(view, View::Dynamic(_))
            && is_true_ci(e.property(view.base(), PLANTUML_SEQUENCE_DIAGRAM_PROPERTY, "false"))
    }

    fn create_diagram(&self, _e: &Engine, view: View<'_>, definition: String) -> Diagram {
        Diagram::new(view.key().to_string(), definition, "puml".to_string())
    }

    fn create_legend(&mut self, _e: &Engine, _view: View<'_>) -> Option<Legend> {
        // The legend renders from the LAST body export's style set — the
        // DN/element width mutation (200) and the legend style only affect
        // the legend's own style block (the body was already serialized).
        let mut w = IndentingWriter::new();
        w.write_line(Some("@startuml"));
        w.write_line(None);
        w.write_line(Some("set separator none"));
        w.write_line(Some("hide stereotype"));
        w.write_line(None);
        w.write_line(Some("<style></style>"));
        w.write_line(None);

        let mut order: Vec<usize> = (0..self.styles.len()).collect();
        order.sort_by(|&a, &b| java_cmp_str(self.styles[a].name(), self.styles[b].name()));

        for &index in &order {
            let PlantUMLStyle::DeploymentNode {
                name, icon, width, ..
            } = &mut self.styles[index]
            else {
                continue;
            };
            *width = Some(200);
            let selector = format!("DeploymentNode-{}", b64(name));
            let description = legend_description(name);
            let icon = icon_leg(icon.as_deref());
            w.write_line(Some(&format!(
                "rectangle \"=={description}{icon}\" <<{selector}>>"
            )));
            w.write_line(None);
        }

        for &index in &order {
            let PlantUMLStyle::Element {
                name,
                shape,
                icon,
                width,
                ..
            } = &mut self.styles[index]
            else {
                continue;
            };
            *width = 200;
            let selector = format!("Element-{}", b64(name));
            let description = legend_description(name);
            let shape = plantuml_shape_of(*shape);
            let icon = icon_leg(icon.as_deref());
            w.write_line(Some(&format!(
                "{shape} \"=={description}{icon}\" <<{selector}>>"
            )));
            w.write_line(None);
        }

        let mut id = 0;
        for &index in &order {
            let PlantUMLStyle::Relationship { name, .. } = &self.styles[index] else {
                continue;
            };
            id += 1;
            let selector = format!("Relationship-{}", b64(name));
            let description = legend_description(name);
            w.write_line(Some(&format!(
                "rectangle \".\" <<.Element-Transparent>> as {id}"
            )));
            w.write_line(Some(&format!(
                "{id} --> {id} <<{selector}>> : \"{description}\""
            )));
            w.write_line(None);
        }

        w.write_line(Some("@enduml"));
        self.add_style(PlantUMLStyle::Legend);
        self.write_styles(&mut w);
        Some(Legend::new(w.to_string()))
    }
}

/// One `PlantUMLStyle` subclass instance (the seven official style classes;
/// each renders through a fresh [`IndentingWriter`] exactly like the
/// official render overrides).
#[derive(Debug, Clone)]
pub enum PlantUMLStyle {
    /// `PlantUMLRootStyle` — `fontName` is always `null` at the call site.
    Root {
        /// `Styles.DEFAULT_BACKGROUND_*`.
        background: String,
        /// `Styles.DEFAULT_COLOR_*`.
        color: String,
    },
    /// `PlantUMLElementStyle`.
    Element {
        /// The merged style's tag (the base64 `name`).
        name: String,
        /// Shape (drives the legend line and `RoundCorner`).
        shape: Option<Shape>,
        /// `MaximumWidth` (always written; the legend rewrites 200).
        width: i32,
        /// Background color.
        background: Option<String>,
        /// Font color.
        color: Option<String>,
        /// Line (stroke) color.
        stroke: Option<String>,
        /// Line thickness.
        stroke_width: i32,
        /// The pre-computed border→dash mapping.
        line_style: String,
        /// Font size.
        font_size: i32,
        /// Icon URL (rendered in the legend, not the block).
        icon: Option<String>,
        /// `plantuml.shadow` property.
        shadow: bool,
    },
    /// `PlantUMLRelationshipStyle`.
    Relationship {
        /// The merged style's tag.
        name: String,
        /// Line AND font color.
        color: Option<String>,
        /// The pre-computed line-style→dash mapping.
        line_style: String,
        /// Line thickness.
        thickness: i32,
        /// Font size.
        font_size: i32,
        /// `MaximumWidth`.
        width: i32,
    },
    /// `PlantUMLDeploymentNodeStyle`.
    DeploymentNode {
        /// The merged style's tag.
        name: String,
        /// Background color.
        background: Option<String>,
        /// Font color.
        color: Option<String>,
        /// Line color.
        stroke: Option<String>,
        /// Line thickness.
        stroke_width: i32,
        /// Pre-computed dash mapping.
        line_style: String,
        /// Font size.
        font_size: i32,
        /// Icon URL (legend lines only).
        icon: Option<String>,
        /// `plantuml.shadow` property.
        shadow: bool,
        /// Legend-only width (`Some` ⇒ `MaximumWidth` line).
        width: Option<i32>,
    },
    /// `PlantUMLBoundaryStyle`.
    Boundary {
        /// The boundary element's name.
        name: String,
        /// Background color.
        background: Option<String>,
        /// Font color.
        color: Option<String>,
        /// Line color.
        stroke: Option<String>,
        /// Line thickness.
        stroke_width: i32,
        /// Pre-computed dash mapping.
        line_style: String,
        /// Font size.
        font_size: i32,
        /// Always false (the synthesized style has no properties).
        shadow: bool,
    },
    /// `PlantUMLGroupStyle`.
    Group {
        /// The raw group name.
        name: String,
        /// Background color.
        background: Option<String>,
        /// Font color.
        color: Option<String>,
        /// Line color.
        stroke: Option<String>,
        /// Line thickness.
        stroke_width: i32,
        /// Pre-computed dash mapping.
        line_style: String,
        /// Font size.
        font_size: i32,
        /// Always false (the synthesized style has no properties).
        shadow: bool,
    },
    /// `PlantUMLLegendStyle` (the transparent legend element).
    Legend,
}

impl PlantUMLStyle {
    /// The class identity for `HashSet` equality (the class-kind key).
    /// check).
    const fn class_tag(&self) -> &'static str {
        match self {
            Self::Root { .. } => "Root",
            Self::Element { .. } => "Element",
            Self::Relationship { .. } => "Relationship",
            Self::DeploymentNode { .. } => "DeploymentNode",
            Self::Boundary { .. } => "Boundary",
            Self::Group { .. } => "Group",
            Self::Legend => "Legend",
        }
    }

    /// The `writeStyles` class-pass index (`Root`, `Element`,
    /// `Relationship`, `DeploymentNode`, `Boundary`, `Group`, `Legend`).
    const fn class_rank(&self) -> u8 {
        match self {
            Self::Root { .. } => 0,
            Self::Element { .. } => 1,
            Self::Relationship { .. } => 2,
            Self::DeploymentNode { .. } => 3,
            Self::Boundary { .. } => 4,
            Self::Group { .. } => 5,
            Self::Legend => 6,
        }
    }

    /// `getName()` — the name-sort key.
    fn name(&self) -> &str {
        match self {
            Self::Root { .. } => ".root",
            Self::Element { name, .. }
            | Self::Relationship { name, .. }
            | Self::DeploymentNode { name, .. }
            | Self::Boundary { name, .. }
            | Self::Group { name, .. } => name,
            Self::Legend => "Element-Transparent",
        }
    }

    /// `getClassSelector()`.
    fn selector(&self) -> String {
        match self {
            Self::Root { .. } => "root".to_string(),
            Self::Element { name, .. } => format!("Element-{}", b64(name)),
            Self::Relationship { name, .. } => format!("Relationship-{}", b64(name)),
            Self::DeploymentNode { name, .. } => format!("DeploymentNode-{}", b64(name)),
            Self::Boundary { name, .. } => format!("Boundary-{}", b64(name)),
            Self::Group { name, .. } => format!("Group-{}", b64(name)),
            Self::Legend => "Element-Transparent".to_string(),
        }
    }

    /// The render — a fresh writer per style: `indent`, the comment,
    /// the selector line, indented fields, `outdent`, `}`, `outdent`,
    /// blank line. One `render_*` fn per official style class; the old shape
    /// was a single 174-line
    /// match behind a `too_many_lines` allow.
    fn render(&self) -> String {
        let mut w = IndentingWriter::new();
        w.indent();
        match self {
            Self::Root { .. } => self.render_root(&mut w),
            Self::Legend => Self::render_legend(&mut w),
            Self::Element { .. } => self.render_element(&mut w),
            Self::Relationship { .. } => self.render_relationship(&mut w),
            Self::DeploymentNode { .. } => self.render_deployment_node(&mut w),
            Self::Boundary { .. } | Self::Group { .. } => self.render_boundary_or_group(&mut w),
        }
        w.outdent();
        w.write_line(Some("}"));
        w.outdent();
        w.write_line(None);
        w.to_string()
    }

    /// Root style — the root class selector renders
    /// WITHOUT a dot.
    fn render_root(&self, w: &mut IndentingWriter) {
        let Self::Root { background, color } = self else {
            return; // only reachable from `render`'s Root arm
        };
        w.write_line(Some("root {"));
        w.indent();
        w.write_line(Some(&format!("BackgroundColor: {background};")));
        w.write_line(Some(&format!("FontColor: {color};")));
    }

    /// Legend style — the transparent legend element.
    fn render_legend(w: &mut IndentingWriter) {
        w.write_line(Some("// transparent element for relationships in legend"));
        w.write_line(Some(".Element-Transparent {"));
        w.indent();
        w.write_line(Some("BackgroundColor: transparent;"));
        w.write_line(Some("LineColor: transparent;"));
        w.write_line(Some("FontColor: transparent;"));
    }

    /// Element style rendering.
    fn render_element(&self, w: &mut IndentingWriter) {
        let Self::Element {
            name,
            shape,
            width,
            background,
            color,
            stroke,
            stroke_width,
            line_style,
            font_size,
            shadow,
            ..
        } = self
        else {
            return; // only reachable from `render`'s Element arm
        };
        w.write_line(Some(&format!("// {name}")));
        w.write_line(Some(&format!(".{} {{", self.selector())));
        w.indent();
        w.write_line(Some(&format!(
            "BackgroundColor: {};",
            color_or_null(background.as_deref())
        )));
        w.write_line(Some(&format!(
            "LineColor: {};",
            color_or_null(stroke.as_deref())
        )));
        w.write_line(Some(&format!("LineStyle: {line_style};")));
        w.write_line(Some(&format!("LineThickness: {stroke_width};")));
        if *shape == Some(Shape::RoundedBox) {
            w.write_line(Some("RoundCorner: 20;"));
        }
        w.write_line(Some(&format!(
            "FontColor: {};",
            color_or_null(color.as_deref())
        )));
        w.write_line(Some(&format!("FontSize: {font_size};")));
        w.write_line(Some("HorizontalAlignment: center;"));
        w.write_line(Some(&format!(
            "Shadowing: {};",
            if *shadow { SHADOW_DISTANCE } else { 0 }
        )));
        w.write_line(Some(&format!("MaximumWidth: {width};")));
    }

    /// Relationship style — the font color mirrors the line
    /// COLOR (one `color` field for both).
    fn render_relationship(&self, w: &mut IndentingWriter) {
        let Self::Relationship {
            name,
            color,
            line_style,
            thickness,
            font_size,
            width,
        } = self
        else {
            return; // only reachable from `render`'s Relationship arm
        };
        w.write_line(Some(&format!("// {name}")));
        w.write_line(Some(&format!(".{} {{", self.selector())));
        w.indent();
        w.write_line(Some(&format!("LineThickness: {thickness};")));
        w.write_line(Some(&format!("LineStyle: {line_style};")));
        w.write_line(Some(&format!(
            "LineColor: {};",
            color_or_null(color.as_deref())
        )));
        w.write_line(Some(&format!(
            "FontColor: {};",
            color_or_null(color.as_deref())
        )));
        w.write_line(Some(&format!("FontSize: {font_size};")));
        w.write_line(Some(&format!("MaximumWidth: {width};")));
    }

    /// Deployment-node style — the max width ONLY when
    /// the legend path set it (`Some`).
    fn render_deployment_node(&self, w: &mut IndentingWriter) {
        let Self::DeploymentNode {
            name,
            background,
            color,
            stroke,
            stroke_width,
            line_style,
            font_size,
            shadow,
            width,
            ..
        } = self
        else {
            return; // only reachable from `render`'s DeploymentNode arm
        };
        w.write_line(Some(&format!("// {name}")));
        w.write_line(Some(&format!(".{} {{", self.selector())));
        w.indent();
        w.write_line(Some(&format!(
            "BackgroundColor: {};",
            color_or_null(background.as_deref())
        )));
        w.write_line(Some(&format!(
            "LineColor: {};",
            color_or_null(stroke.as_deref())
        )));
        w.write_line(Some(&format!("LineStyle: {line_style};")));
        w.write_line(Some(&format!("LineThickness: {stroke_width};")));
        w.write_line(Some(&format!(
            "FontColor: {};",
            color_or_null(color.as_deref())
        )));
        w.write_line(Some(&format!("FontSize: {font_size};")));
        w.write_line(Some("HorizontalAlignment: center;"));
        w.write_line(Some(&format!(
            "Shadowing: {};",
            if *shadow { SHADOW_DISTANCE } else { 0 }
        )));
        if let Some(width) = width {
            w.write_line(Some(&format!("MaximumWidth: {width};")));
        }
    }

    /// Boundary style and group style —
    /// the two classes render the same field set (the old single
    /// or-pattern arm), only the selector differs.
    fn render_boundary_or_group(&self, w: &mut IndentingWriter) {
        let (Self::Boundary {
            name,
            background,
            color,
            stroke,
            stroke_width,
            line_style,
            font_size,
            shadow,
        }
        | Self::Group {
            name,
            background,
            color,
            stroke,
            stroke_width,
            line_style,
            font_size,
            shadow,
        }) = self
        else {
            return; // only reachable from `render`'s Boundary|Group arm
        };
        w.write_line(Some(&format!("// {name}")));
        w.write_line(Some(&format!(".{} {{", self.selector())));
        w.indent();
        w.write_line(Some(&format!(
            "BackgroundColor: {};",
            color_or_null(background.as_deref())
        )));
        w.write_line(Some(&format!(
            "LineColor: {};",
            color_or_null(stroke.as_deref())
        )));
        w.write_line(Some(&format!("LineStyle: {line_style};")));
        w.write_line(Some(&format!("LineThickness: {stroke_width};")));
        w.write_line(Some(&format!(
            "FontColor: {};",
            color_or_null(color.as_deref())
        )));
        w.write_line(Some(&format!("FontSize: {font_size};")));
        w.write_line(Some("HorizontalAlignment: center;"));
        w.write_line(Some(&format!(
            "Shadowing: {};",
            if *shadow { SHADOW_DISTANCE } else { 0 }
        )));
    }
}

/// The synthesized boundary/group `ElementStyle("")` shape (numbers always
/// resolved; shadow never read from here — the callers pass `false`).
struct ElementSpec {
    background: Option<String>,
    stroke: Option<String>,
    stroke_width: i32,
    border: Option<LineStyle>,
    color: Option<String>,
    icon: Option<String>,
    font_size: i32,
}

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
fn metadata_font_size(font_size: i32) -> i32 {
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
fn field_str<'a>(
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
fn field_i32(
    first: Option<&ElementStyle>,
    second: Option<&ElementStyle>,
    third: Option<&ElementStyle>,
    get: fn(&ElementStyle) -> Option<i32>,
) -> Option<i32> {
    [first, second, third].into_iter().flatten().find_map(get)
}

/// The first non-null border across the chain.
fn field_border(
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
fn field_icon(
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
fn line_style(border: Option<LineStyle>, width: i32) -> String {
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
fn shadow_of(style: &ElementStyle) -> bool {
    style
        .properties
        .get(PLANTUML_SHADOW_PROPERTY)
        .is_some_and(is_true_ci)
}

/// The legend description: `name`, minus the `Element,` / `Relationship,`
/// prefix, with `replaceAll(",", ", ")`.
fn legend_description(name: &str) -> String {
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
fn icon_leg(icon: Option<&str>) -> String {
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
fn id_leaf(e: &Engine, element: ElemRef<'_>) -> Result<String, ExportError> {
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
fn b64(input: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use oxidizr_core::configuration::Workspace;
    use oxidizr_core::docs::Documentation;
    use oxidizr_core::model::{ModelItem, Person, Relationship, SoftwareSystem};
    use oxidizr_core::properties::Properties;
    use oxidizr_core::tags::Tags;
    use oxidizr_core::views::{
        ElementView, RelationshipView, SystemLandscapeView, ViewBase, ViewContent,
    };

    fn item(id: &str, name: &str) -> ModelItem {
        ModelItem::new(Id::new(id), name.to_string())
    }

    fn person(id: &str, name: &str) -> Person {
        Person {
            item: item(id, name),
            group: None,
            relationships: Vec::new(),
        }
    }

    fn system(id: &str, name: &str) -> SoftwareSystem {
        SoftwareSystem {
            item: item(id, name),
            group: None,
            containers: Vec::new(),
            documentation: Documentation::default(),
            relationships: Vec::new(),
        }
    }

    fn rel(id: &str, source: &str, destination: &str, description: Option<&str>) -> Relationship {
        Relationship {
            id: Id::new(id),
            source: Id::new(source),
            destination: Id::new(destination),
            description: description.map(str::to_string),
            technology: None,
            tags: Tags::default(),
            url: None,
            properties: Properties::default(),
            perspectives: Vec::new(),
            interaction_style: None,
            linked_relationship_id: None,
        }
    }

    fn ev(id: &str) -> ElementView {
        ElementView {
            id: Id::new(id),
            x: 0,
            y: 0,
        }
    }

    fn rv(id: &str) -> RelationshipView {
        RelationshipView {
            id: Id::new(id),
            description: None,
            url: None,
            order: None,
            response: None,
            properties: Properties::default(),
            vertices: Vec::new(),
            routing: None,
            jump: None,
            position: None,
        }
    }

    /// Landscape view: person `1` (Alice) → system `2` (Big Bank) via
    /// relationship `5` ("Uses").
    fn workspace() -> Workspace {
        let mut workspace = Workspace::default();
        workspace.model.people = vec![person("1", "Alice")];
        workspace.model.software_systems = vec![system("2", "Big Bank")];
        workspace.model.relationships = vec![rel("5", "1", "2", Some("Uses"))];
        let mut view = SystemLandscapeView {
            base: ViewBase::new("SystemLandscape-001", 1),
            content: ViewContent::default(),
            enterprise_boundary_visible: true,
            animations: Vec::new(),
        };
        view.content.elements = vec![ev("1"), ev("2")];
        view.content.relationships = vec![rv("5")];
        workspace.views.system_landscape_views.push(view);
        workspace
    }

    fn body(workspace: &Workspace, scheme: ColorScheme) -> Diagram {
        let mut engine = Engine::new(workspace, scheme);
        engine
            .export_all(&mut StructurizrDialect::new())
            .expect("export")[0]
            .clone()
    }

    /// The pinned no-theme light document: header legs, the name-sorted
    /// Root/Element/Relationship style block replacing the placeholder,
    /// the `==`-prefixed element legs (metadata font floor(24x0.7)=16)
    /// and the `-->` relationship leg.
    #[test]
    fn landscape_light_full_document() {
        let diagram = body(&workspace(), ColorScheme::Light);
        assert_eq!(
            diagram.definition(),
            "@startuml\ntitle <size:24>System Landscape View</size>\n\nset separator none\ntop to bottom direction\nhide stereotype\n\n<style>\n  root {\n    BackgroundColor: #ffffff;\n    FontColor: #444444;\n  }\n  // Element\n  .Element-RWxlbWVudA== {\n    BackgroundColor: #ffffff;\n    LineColor: #444444;\n    LineStyle: 0;\n    LineThickness: 2;\n    FontColor: #444444;\n    FontSize: 24;\n    HorizontalAlignment: center;\n    Shadowing: 0;\n    MaximumWidth: 450;\n  }\n  // Relationship\n  .Relationship-UmVsYXRpb25zaGlw {\n    LineThickness: 2;\n    LineStyle: 10-10;\n    LineColor: #444444;\n    FontColor: #444444;\n    FontSize: 24;\n    MaximumWidth: 200;\n  }\n</style>\n\nrectangle \"==Alice\\n<size:16>[Person]</size>\" <<Element-RWxlbWVudA==>> as Alice\nrectangle \"==Big Bank\\n<size:16>[Software System]</size>\" <<Element-RWxlbWVudA==>> as BigBank\n\nAlice --> BigBank <<Relationship-UmVsYXRpb25zaGlw>> : \"Uses\"\n\n@enduml"
        );
    }

    /// The legend: the DN/element passes rewrite widths to 200, the
    /// element descriptions drop the `Element,` prefix, the relationship
    /// pass numbers from 1 with the transparent `<<.Element-Transparent>>`
    /// rectangles and the Legend style is APPENDED before `writeStyles`.
    #[test]
    fn landscape_light_legend() {
        let diagram = body(&workspace(), ColorScheme::Light);
        let legend = diagram.legend().expect("legend");
        assert_eq!(
            legend.definition(),
            "@startuml\n\nset separator none\nhide stereotype\n\n<style>\n  root {\n    BackgroundColor: #ffffff;\n    FontColor: #444444;\n  }\n  // Element\n  .Element-RWxlbWVudA== {\n    BackgroundColor: #ffffff;\n    LineColor: #444444;\n    LineStyle: 0;\n    LineThickness: 2;\n    FontColor: #444444;\n    FontSize: 24;\n    HorizontalAlignment: center;\n    Shadowing: 0;\n    MaximumWidth: 200;\n  }\n  // Relationship\n  .Relationship-UmVsYXRpb25zaGlw {\n    LineThickness: 2;\n    LineStyle: 10-10;\n    LineColor: #444444;\n    FontColor: #444444;\n    FontSize: 24;\n    MaximumWidth: 200;\n  }\n  // transparent element for relationships in legend\n  .Element-Transparent {\n    BackgroundColor: transparent;\n    LineColor: transparent;\n    FontColor: transparent;\n  }\n</style>\n\nrectangle \"==Element\" <<Element-RWxlbWVudA==>>\n\nrectangle \".\" <<.Element-Transparent>> as 1\n1 --> 1 <<Relationship-UmVsYXRpb25zaGlw>> : \"Relationship\"\n\n@enduml"
        );
    }

    /// `plantuml.title=false` drops the title block; the dark scheme
    /// renders `Styles.DEFAULT_*_DARK` roots (and the merged-style
    /// background falls to `#111111`).
    #[test]
    fn dark_scheme_title_off() {
        let mut ws = workspace();
        ws.views.system_landscape_views[0]
            .base
            .properties
            .insert(PLANTUML_TITLE_PROPERTY, "false");
        let definition = body(&ws, ColorScheme::Dark).definition().to_string();
        assert!(definition.starts_with("@startuml\n\nset separator none\n"));
        assert!(
            definition
                .contains("  root {\n    BackgroundColor: #111111;\n    FontColor: #cccccc;\n  }")
        );
        assert!(definition.contains("BackgroundColor: #111111;\n    LineColor: #cccccc;"));
    }

    /// `AutomaticLayout`: only `LeftRight` leaves the default direction;
    /// separations divide by 5 and 10 with integer division.
    #[test]
    fn automatic_layout_legs() {
        let mut ws = workspace();
        let view = &mut ws.views.system_landscape_views[0];
        view.content.automatic_layout = Some(oxidizr_core::views::AutomaticLayout {
            rank_direction: Some(RankDirection::TopBottom),
            rank_separation: 100,
            node_separation: 50,
            ..Default::default()
        });
        let definition = body(&ws, ColorScheme::Light).definition().to_string();
        assert!(
            definition
                .contains("top to bottom direction\nskinparam ranksep 20\nskinparam nodesep 5\n")
        );
        let view = &mut ws.views.system_landscape_views[0];
        view.content
            .automatic_layout
            .as_mut()
            .unwrap()
            .rank_direction = Some(RankDirection::LeftRight);
        let definition = body(&ws, ColorScheme::Light).definition().to_string();
        assert!(definition.contains("left to right direction\nskinparam ranksep 20\n"));
    }

    /// Sequence diagrams: `!pragma teoz true` replaces the direction leg
    /// (boundaries default true), elements take the sequence shape with
    /// the label-first form, relationships the unquoted `->` arrow.
    #[test]
    fn sequence_diagram() {
        let mut ws = Workspace::default();
        ws.model.people = vec![person("1", "Alice"), person("2", "Bob")];
        ws.model.relationships = vec![rel("5", "1", "2", Some("Uses"))];
        let mut view = oxidizr_core::views::DynamicView {
            base: ViewBase::new("dynamic", 1),
            content: ViewContent::default(),
            element_id: None,
            deployment_environment: None,
            external_boundaries_visible: false,
        };
        view.base
            .properties
            .insert(PLANTUML_SEQUENCE_DIAGRAM_PROPERTY, "true");
        view.content.elements = vec![ev("1"), ev("2")];
        view.content.relationships = vec![rv("5")];
        ws.views.dynamic_views.push(view);
        let definition = body(&ws, ColorScheme::Light).definition().to_string();
        assert!(definition.starts_with("@startuml\ntitle <size:24>Dynamic View</size>\n\nset separator none\n!pragma teoz true\nhide stereotype\n"));
        assert!(definition.contains(
            "participant \"Alice\\n<size:16>[Person]</size>\" as Alice <<Element-RWxlbWVudA==>>\n"
        ));
        assert!(definition.contains("Alice -> Bob <<Relationship-UmVsYXRpb25zaGlw>> : Uses\n"));
        assert!(!definition.contains("top to bottom direction"));
    }

    /// Nested groups: the display name cuts at the LAST separator, the
    /// selector base64s the FULL group, the rectangles nest `as groupN`
    /// with the exporter-scoped counter, and the synthesized group style
    /// is dotted (`2-2`) with `Shadowing: 0`.
    #[test]
    fn nested_groups() {
        let mut ws = workspace();
        ws.model
            .properties
            .insert("structurizr.groupSeparator", "/");
        ws.model.people[0].group = Some("x/y".to_string());
        let definition = body(&ws, ColorScheme::Light).definition().to_string();
        assert!(definition.contains("rectangle \"x\" <<Group-eA==>> as group1 {\n    rectangle \"y\" <<Group-eC95>> as group2 {\n      rectangle \"==Alice"));
        assert!(definition.contains("  // x/y\n  .Group-eC95 {\n    BackgroundColor: #ffffff;\n    LineColor: #444444;\n    LineStyle: 2-2;\n    LineThickness: 2;\n    FontColor: #444444;\n    FontSize: 24;\n    HorizontalAlignment: center;\n    Shadowing: 0;\n  }"));
    }

    /// Container views close NON-container elements first, then write
    /// `rectamgle` boxes per in-scope system whose containers are shown
    /// (boundary chain: name + type line, `Boundary-<b64(name)>`).
    #[test]
    fn container_boundary() {
        let mut ws = Workspace::default();
        let mut sys = system("2", "Big Bank");
        sys.containers = vec![oxidizr_core::model::Container {
            item: item("3", "Web App"),
            group: None,
            technology: Some("Java".to_string()),
            components: Vec::new(),
            documentation: Documentation::default(),
            relationships: Vec::new(),
        }];
        ws.model.software_systems = vec![sys];
        let mut view = oxidizr_core::views::ContainerView {
            base: ViewBase::new("container", 1),
            content: ViewContent::default(),
            software_system_id: Id::new("2"),
            external_software_system_boundaries_visible: false,
            animations: Vec::new(),
        };
        view.base.title = Some("Container View".to_string());
        view.content.elements = vec![ev("2"), ev("3")];
        ws.views.container_views.push(view);
        let definition = body(&ws, ColorScheme::Light).definition().to_string();
        assert!(definition.contains("rectangle \"Big Bank\\n<size:16>[Software System]</size>\" <<Boundary-QmlnIEJhbms=>> {\n  rectangle \"==Web App\\n<size:16>[Container: Java]</size>\" <<Element-RWxlbWVudA==>> as BigBank.WebApp\n}\n"));
    }

    /// Deployment nodes: `filter(environment).filter(name)` ids, the
    /// `DeploymentNode-` selector off the merged TAG, the instance-count
    /// suffix gate (`"1"` writes nothing) and the legend width rewrite.
    #[test]
    fn deployment_node_and_legend() {
        let mut ws = Workspace::default();
        ws.model.deployment_nodes = vec![oxidizr_core::model::DeploymentNode {
            item: item("1", "Server"),
            group: None,
            environment: "Default".to_string(),
            technology: Some("AWS EC2".to_string()),
            instances: "1".to_string(),
            deployment_groups: Vec::new(),
            children: Vec::new(),
            infrastructure_nodes: Vec::new(),
            software_system_instances: Vec::new(),
            container_instances: Vec::new(),
            relationships: Vec::new(),
        }];
        let mut view = oxidizr_core::views::DeploymentView {
            base: ViewBase::new("deployment", 1),
            content: ViewContent::default(),
            environment: "Default".to_string(),
            software_system_id: None,
            animations: Vec::new(),
        };
        view.base.title = Some("Deployment View".to_string());
        view.content.elements = vec![ev("1")];
        ws.views.deployment_views.push(view);
        let diagram = body(&ws, ColorScheme::Light);
        let definition = diagram.definition();
        assert!(definition.contains("rectangle \"Server\\n<size:16>[Deployment Node: AWS EC2]</size>\" <<DeploymentNode-RWxlbWVudA==>> as Default.Server {\n}\n"));
        let legend = diagram.legend().expect("legend").definition().to_string();
        assert!(legend.contains("MaximumWidth: 200;\n  }\n  // transparent"));
        assert!(legend.contains("rectangle \"==Element\" <<DeploymentNode-RWxlbWVudA==>>"));
    }

    /// A `plantuml.skinparams` entry whose value splits empty is the official tooling
    /// second-part index-out-of-range — surfaced as `ReferenceAbort`.
    #[test]
    fn skin_param_empty_value_aborts() {
        let mut ws = workspace();
        ws.views.system_landscape_views[0]
            .base
            .properties
            .insert(PLANTUML_SKINPARAMS_PROPERTY, "foo=");
        let mut engine = Engine::new(&ws, ColorScheme::Light);
        let err = engine
            .export_all(&mut StructurizrDialect::new())
            .expect_err("abort");
        assert!(matches!(
            err,
            ExportError::ReferenceAbort(
                "skinparam with an empty value (Java split(\"=\")[1] AIOOBE)"
            )
        ));
    }

    #[test]
    fn style_helpers() {
        assert_eq!(b64("Element"), "RWxlbWVudA==");
        assert_eq!(b64("Relationship"), "UmVsYXRpb25zaGlw");
        assert_eq!(b64(""), "");
        assert_eq!(filter("Big Bank!"), "BigBank");
        assert_eq!(line_style(Some(LineStyle::Dotted), 2), "2-2");
        assert_eq!(line_style(Some(LineStyle::Dashed), 2), "10-10");
        assert_eq!(line_style(None, 3), "0");
        assert_eq!(metadata_font_size(24), 16);
        assert_eq!(legend_description("Element,Person, foo"), "Person,  foo");
        assert_eq!(legend_description("Relationship"), "Relationship");
        assert_eq!(java_split("foo=", '='), vec!["foo"]);
    }

    #[test]
    fn group_display_name_cuts_like_java_substring() {
        assert_eq!(
            StructurizrDialect::group_display_name("x/y", Some("/")).unwrap(),
            "y"
        );
        assert_eq!(
            StructurizrDialect::group_display_name("y", Some("/")).unwrap(),
            "y"
        );
        // ABSENT separator: the official code still cuts at length minus one.
        assert_eq!(
            StructurizrDialect::group_display_name("ab", Some("//")).unwrap(),
            "b"
        );
        assert_eq!(
            StructurizrDialect::group_display_name("ab", None).unwrap(),
            "ab"
        );
        // A cut at exactly the length is valid (start == length): empty.
        assert_eq!(
            StructurizrDialect::group_display_name("a", Some("//")).unwrap(),
            ""
        );
        // start 2 > length 1: the official index abort.
        assert!(StructurizrDialect::group_display_name("a", Some("///")).is_err());
    }
}
