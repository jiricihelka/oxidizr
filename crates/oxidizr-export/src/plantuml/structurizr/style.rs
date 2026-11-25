//! The seven official `PlantUML*Style` renderers and the synthesized
//! boundary/group `ElementSpec` shape they render from. The dialect hooks
//! and the engine legs live in the parent module; the shared free helpers
//! resolve through `super::`.

use oxidizr_core::views::{LineStyle, Shape};

use crate::indent::IndentingWriter;

use super::{SHADOW_DISTANCE, b64, color_or_null};

/// One `PlantUMLStyle` subclass instance (the seven official style classes;
/// each renders through a fresh [`IndentingWriter`] exactly like the
/// official render overrides).
#[derive(Debug, Clone)]
pub(crate) enum PlantUMLStyle {
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
    pub(super) const fn class_tag(&self) -> &'static str {
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
    pub(super) const fn class_rank(&self) -> u8 {
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
    pub(super) fn name(&self) -> &str {
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
    pub(super) fn selector(&self) -> String {
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
    pub(super) fn render(&self) -> String {
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
pub(super) struct ElementSpec {
    pub(super) background: Option<String>,
    pub(super) stroke: Option<String>,
    pub(super) stroke_width: i32,
    pub(super) border: Option<LineStyle>,
    pub(super) color: Option<String>,
    pub(super) icon: Option<String>,
    pub(super) font_size: i32,
}
