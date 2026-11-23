//! The Mermaid dialect — the official Mermaid exporter port.
//!
//! The shared engine loops (view iteration, grouped
//! element walk, deployment-node recursion, relationship ordering) are the
//! shared engine; this dialect owns the per-diagram `groupId` counter and
//! the line formats: the `graph <D>` header (direction from
//! `automaticLayout` only), the double-indent `subgraph diagram` wrapper,
//! the `group<N>` boundaries with the `Group:`/`Group`/`#cccccc` color
//! chain, the div-label elements with the 30-column greedy `lines()` wrap,
//! the DeploymentNode-endpoint relationship skip, and the OWNED sequence
//! document: a standalone `sequenceDiagram` — NOT the engine's
//! `AbstractPlantUMLExporter` sequence walk — reached through the
//! [`Dialect::sequence_document`] route. Frames never (base
//! `isAnimationSupported` is `false`); legend never (`createLegend` is
//! null); file extension `mmd` (`MermaidDiagram.getFileExtension`).

use oxidizr_core::validate::ElementKind;
use oxidizr_core::views::{
    DynamicView, ElementStyle, LineStyle, RankDirection, RelationshipView, Shape, View,
};

use crate::diagram::Diagram;
use crate::error::ExportError;
use crate::indent::IndentingWriter;
use crate::plantuml::engine::{Dialect, Engine, is_null_or_empty, is_value, utf16_len};
use crate::plantuml::model::{ElemRef, element_by_id};
use crate::plantuml::structurizr::{
    StructurizrDialect, automatic_layout, color_or_null, element_style_of, view_name,
};

/// `MermaidDiagramExporter.MERMAID_TITLE_PROPERTY` (default `true`,
/// case-SENSITIVE equality check.
pub const MERMAID_TITLE_PROPERTY: &str = "mermaid.title";
/// `MERMAID_SEQUENCE_DIAGRAM_PROPERTY` (default `false`,
/// case-insensitive).
pub const MERMAID_SEQUENCE_DIAGRAM_PROPERTY: &str = "mermaid.sequenceDiagram";
/// `MERMAID_ICONS_PROPERTY` (default `false`, case-SENSITIVE `equals`).
pub const MERMAID_ICONS_PROPERTY: &str = "mermaid.icons";

/// The `mermaid.*` gate helpers — per-property case rules (the export catalog):
/// The title and icons need exactly `"true"` (case-SENSITIVE), the sequence
/// gate ignores case.
fn is_title_on<'a>(e: &Engine<'a>, view: View<'a>) -> bool {
    e.property(view.base(), MERMAID_TITLE_PROPERTY, "true") == "true"
}

fn sequence_gated<'a>(e: &Engine<'a>, view: View<'a>) -> bool {
    e.property(view.base(), MERMAID_SEQUENCE_DIAGRAM_PROPERTY, "false")
        .eq_ignore_ascii_case("true")
}

fn icons_gated<'a>(e: &Engine<'a>, view: View<'a>) -> bool {
    e.property(view.base(), MERMAID_ICONS_PROPERTY, "false") == "true"
}

/// Icon support: non-empty AND starts with `http` (no
/// icon-scale check here — that quirk is `PlantUML`-only).
fn has_supported_icon(style: &ElementStyle) -> bool {
    style
        .icon
        .as_deref()
        .is_some_and(|icon| is_value(icon) && icon.starts_with("http"))
}

/// `escapeSemicolon`: semicolons break sequence diagrams
/// (<https://github.com/mermaid-js/mermaid/issues/1445>). A null `s` is
/// the official tooling null-pointer abort, so the caller unwraps first.
fn escape_semicolon(s: &str) -> String {
    s.replace(';', "#59;")
}

/// `lines(text)`: trim, split on whitespace runs, greedy lines while
/// `line.length() + word.length() + 1 < 30` (UTF-16 units, like the official tooling’s
/// `String.length()`), `<br />` between lines.
fn lines(text: &str) -> String {
    let mut buf = String::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if line.is_empty() {
            line.push_str(word);
        } else if utf16_len(&line) + utf16_len(word) + 1 < 30 {
            line.push(' ');
            line.push_str(word);
        } else {
            buf.push_str(&line);
            buf.push_str("<br />");
            line.clear();
            line.push_str(word);
        }
    }
    if !line.is_empty() {
        buf.push_str(&line);
    }
    buf
}

/// The node symbols by resolved shape: `RoundedBox` → `( … )`, `Cylinder`
/// → `[( … )]`, anything else (incl. `Person`/`Pipe`!) → `[ … ]`.
fn node_symbols(shape: Option<Shape>) -> (&'static str, &'static str) {
    match shape {
        Some(Shape::RoundedBox) => ("(", ")"),
        Some(Shape::Cylinder) => ("[(", ")]"),
        _ => ("[", "]"),
    }
}

/// The Mermaid dialect: the engine drives the iteration, the dialect owns
/// the group-id counter (reset in the header write, one per diagram).
#[derive(Debug, Default)]
pub struct MermaidDialect {
    group_id: u32,
}

impl MermaidDialect {
    /// A fresh dialect (the group-id counter starts at 0; the header resets
    /// it per body export exactly like the official counter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The system/container boundary body (`MMD:120-128` / `138-146`): the
    /// resolved STROKE renders as both `stroke:` and `color:`.
    fn write_boundary<'a>(e: &Engine<'a>, element: ElemRef<'a>, w: &mut IndentingWriter) {
        // The abort leg of style resolution is unreachable: a boundary
        // element the engine resolved is never an instance (c4.rs idiom).
        let style = element_style_of(e, element).unwrap_or_default();
        let color = color_or_null(style.stroke.as_deref());
        w.write_line(Some(&format!(
            "subgraph {} [\"{}\"]",
            element.id(),
            element.name()
        )));
        w.indent();
        w.write_line(Some(&format!(
            "style {} fill:#ffffff,stroke:{color},color:{color}",
            element.id()
        )));
        w.write_line(None);
    }

    /// `startGroupBoundary`'s color chain: `findElementStyle("Group:" +
    /// group)` when it carries a color, else `findElementStyle("Group")`
    /// likewise, else `#cccccc`.
    fn group_color(e: &Engine, group: &str) -> String {
        let styles = &e.configuration().styles;
        let scheme = e.color_scheme();
        let by_group = styles.find_element_style(&format!("Group:{group}"), scheme);
        let chosen = match by_group {
            Some(ref style) if style.color.as_deref().is_some_and(is_value) => by_group,
            _ => styles.find_element_style("Group", scheme),
        };
        chosen
            .and_then(|style| style.color)
            .filter(|color| is_value(color))
            .unwrap_or_else(|| "#cccccc".to_string())
    }

    /// The participant leg: `actor` iff the resolved shape is `Person`;
    /// the `<br />`-prefixed type part empties when the type is empty OR
    /// metadata is off (unlike the element leg, which wraps empty types).
    fn write_participant<'a>(
        e: &Engine<'a>,
        _view: View<'a>,
        element: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let style = element_style_of(e, element)?;
        let keyword = if style.shape == Some(Shape::Person) {
            "actor"
        } else {
            "participant"
        };
        let type_of = e.type_of(element, true);
        let type_part = if type_of.is_empty() || style.metadata == Some(false) {
            String::new()
        } else {
            format!("<br />{type_of}")
        };
        w.write_line(Some(&format!(
            "{keyword} {} as {}{type_part}",
            element.id(),
            element.name()
        )));
        Ok(())
    }
}

impl Dialect for MermaidDialect {
    fn write_header<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        self.group_id = 0;
        // Direction comes from `automaticLayout.rankDirection` only; a
        // present layout with a null rank direction is the official
        // null-pointer crash (verified).
        let direction = match automatic_layout(view) {
            None => "TB",
            Some(layout) => match layout.rank_direction {
                Some(RankDirection::TopBottom) => "TB",
                Some(RankDirection::BottomTop) => "BT",
                Some(RankDirection::LeftRight) => "LR",
                Some(RankDirection::RightLeft) => "RL",
                None => {
                    return Err(ExportError::ReferenceAbort(
                        "automatic layout without a rank direction (Java switch NPE)",
                    ));
                }
            },
        };
        w.write_line(Some(&format!("graph {direction}")));
        w.indent();
        w.write_line(Some("linkStyle default fill:#ffffff"));
        w.write_line(None);

        let view_title = if is_title_on(e, view) {
            match view.base().title.as_deref() {
                Some(title) if is_value(title) => title.to_string(),
                // The view-name fallback — the official tooling crashes on a
                // dangling scoped view (engine `view_name`).
                _ => view_name(e, view)?,
            }
        } else {
            " ".to_string()
        };
        w.write_line(Some(&format!("subgraph diagram [\"{view_title}\"]")));
        w.indent();
        w.write_line(Some("style diagram fill:#ffffff,stroke:#ffffff"));
        w.write_line(None);
        Ok(())
    }

    fn write_footer<'a>(&mut self, _e: &Engine<'a>, _view: View<'a>, w: &mut IndentingWriter) {
        // ONE `end` closes the two opens (header).
        w.outdent();
        w.write_line(Some("end"));
        w.outdent();
    }

    fn write_element<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        element: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        // Style resolves on the ORIGINAL element (instances keep their own
        // tags); name/description/type unwrap from the instance.
        let style = element_style_of(e, element)?;
        let written = match element.kind() {
            ElementKind::SoftwareSystemInstance | ElementKind::ContainerInstance => element
                .target(e.model())
                .ok_or(ExportError::ReferenceAbort(
                    "instance references a missing element (Java getElement() NPE)",
                ))?,
            _ => element,
        };
        let (open, close) = node_symbols(style.shape);

        let description =
            if is_null_or_empty(written.description()) || style.description == Some(false) {
                String::new()
            } else {
                format!(
                    "<div style='font-size: 80%; margin-top:10px'>{}</div>",
                    lines(written.description().unwrap_or_default())
                )
            };
        // No emptiness gate on the type here (unlike the sequence leg):
        // metadata on, the div wraps even an empty type.
        let type_of = if style.metadata == Some(false) {
            String::new()
        } else {
            format!(
                "<div style='font-size: 70%; margin-top: 0px'>{}</div>",
                e.type_of(written, true)
            )
        };
        let icon = if icons_gated(e, view) && has_supported_icon(&style) {
            format!(
                "<div><img src='{}' style='max-height: 50px; margin: auto; margin-top:10px'/></div>",
                style.icon.as_deref().unwrap_or_default()
            )
        } else {
            String::new()
        };

        w.write_line(Some(&format!(
            "{}{open}\"<div style='font-weight: bold'>{}</div>{}{}{}\"{close}",
            element.id(),
            written.name(),
            type_of,
            description,
            icon
        )));

        if let Some(url) = element.url().filter(|url| is_value(url)) {
            w.write_line(Some(&format!("click {} {url} \"{url}\"", element.id())));
        }
        // Both official branches (instance / not) are identical.
        w.write_line(Some(&format!(
            "style {} fill:{},stroke:{},color:{}",
            element.id(),
            color_or_null(style.background.as_deref()),
            color_or_null(style.stroke.as_deref()),
            color_or_null(style.color.as_deref())
        )));
        Ok(())
    }

    fn write_relationship<'a>(
        &mut self,
        e: &Engine<'a>,
        _view: View<'a>,
        relationship_view: &'a RelationshipView,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let relationship = e.relationship(relationship_view)?;
        let dangling =
            || ExportError::ReferenceAbort("relationship endpoint does not resolve (Java NPE)");
        let mut source = element_by_id(e.model(), &relationship.source).ok_or_else(dangling)?;
        let mut destination =
            element_by_id(e.model(), &relationship.destination).ok_or_else(dangling)?;

        // Any DeploymentNode endpoint kills the edge.
        if source.kind() == ElementKind::DeploymentNode
            || destination.kind() == ElementKind::DeploymentNode
        {
            return Ok(());
        }

        if relationship_view.response == Some(true) {
            std::mem::swap(&mut source, &mut destination);
        }

        let style =
            e.configuration()
                .styles
                .relationship_style(relationship, e.model(), e.color_scheme());
        let solid = style.style == Some(LineStyle::Solid) || style.dashed == Some(false);
        let dash = if solid { "-" } else { "." };

        let mut description: Option<String> = relationship_view
            .description
            .as_deref()
            .filter(|d| is_value(d))
            .or(relationship.description.as_deref())
            .map(str::to_owned);
        if let Some(order) = relationship_view.order.as_deref().filter(|o| is_value(o)) {
            // The official string concat: a null description renders the literal
            // `null` inside the `{order}. ` prefix.
            description = Some(format!(
                "{order}. {}",
                description.as_deref().unwrap_or("null")
            ));
        }

        let technology = match relationship.technology.as_deref() {
            Some(technology) if is_value(technology) => format!("[{technology}]"),
            _ => String::new(),
        };
        w.write_line(Some(&format!(
            "{}-{dash} \"<div>{}</div><div style='font-size: 70%'>{}</div>\" {dash}->{}",
            source.id(),
            lines(description.as_deref().unwrap_or_default()),
            technology,
            destination.id()
        )));
        Ok(())
    }

    fn start_group_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        _view: View<'a>,
        group: &str,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        self.group_id += 1;
        // Same `lastIndexOf` cut as the `Structurizr` dialect (the engine
        // helper models the official index-cut abort leg).
        let group_name = StructurizrDialect::group_display_name(group, Some(&e.group_separator()))?;
        let color = Self::group_color(e, group);
        w.write_line(Some(&format!(
            "subgraph group{} [\"{group_name}\"]",
            self.group_id
        )));
        w.indent();
        w.write_line(Some(&format!(
            "style group{} fill:#ffffff,stroke:{color},color:{color},stroke-dasharray:5",
            self.group_id
        )));
        w.write_line(None);
        Ok(())
    }

    fn end_group_boundary<'a>(
        &mut self,
        _e: &Engine<'a>,
        _view: View<'a>,
        w: &mut IndentingWriter,
    ) {
        w.outdent();
        w.write_line(Some("end"));
        w.write_line(None);
    }

    fn start_software_system_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        _view: View<'a>,
        system: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) {
        Self::write_boundary(e, system, w);
    }

    fn end_software_system_boundary<'a>(
        &mut self,
        _e: &Engine<'a>,
        _view: View<'a>,
        w: &mut IndentingWriter,
    ) {
        w.outdent();
        w.write_line(Some("end"));
        w.write_line(None);
    }

    fn start_container_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        _view: View<'a>,
        container: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) {
        Self::write_boundary(e, container, w);
    }

    fn end_container_boundary<'a>(
        &mut self,
        _e: &Engine<'a>,
        _view: View<'a>,
        w: &mut IndentingWriter,
    ) {
        w.outdent();
        w.write_line(Some("end"));
        w.write_line(None);
    }

    fn start_deployment_node_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        _view: View<'a>,
        node: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        // The deployment-node boundary carries stroke AND color (unlike the
        // system/container boundary's stroke-as-font quirk).
        let style = element_style_of(e, node)?;
        w.write_line(Some(&format!(
            "subgraph {} [\"{}\"]",
            node.id(),
            node.name()
        )));
        w.indent();
        w.write_line(Some(&format!(
            "style {} fill:#ffffff,stroke:{},color:{}",
            node.id(),
            color_or_null(style.stroke.as_deref()),
            color_or_null(style.color.as_deref())
        )));
        w.write_line(None);
        Ok(())
    }

    fn end_deployment_node_boundary<'a>(
        &mut self,
        _e: &Engine<'a>,
        _view: View<'a>,
        w: &mut IndentingWriter,
    ) {
        w.outdent();
        w.write_line(Some("end"));
        w.write_line(None);
    }

    // The `AbstractPlantUMLExporter` box hooks: the base class the Mermaid
    // exporter extends has none, and the owned sequence document
    // bypasses the engine's sequence walk — so they stay no-ops.
    fn start_boundary<'a>(
        &mut self,
        _e: &Engine<'a>,
        _view: View<'a>,
        _element: ElemRef<'a>,
        _w: &mut IndentingWriter,
    ) {
    }

    fn end_boundary<'a>(&mut self, _e: &Engine<'a>, _view: View<'a>, _w: &mut IndentingWriter) {}

    fn render_as_sequence_diagram<'a>(&self, e: &Engine<'a>, view: View<'a>) -> bool {
        sequence_gated(e, view)
    }

    fn owns_sequence_document(&self) -> bool {
        true
    }

    /// `MermaidDiagramExporter.export(DynamicView)`'s sequence branch
    /// (the export catalog): a standalone `sequenceDiagram`, no header/footer, no
    /// frames, no legend.
    fn sequence_document<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        _dv: &'a DynamicView,
    ) -> Result<Diagram, ExportError> {
        let mut w = IndentingWriter::new();
        w.write_line(Some("sequenceDiagram"));
        w.write_line(None);
        w.indent();

        // LinkedHashSet over the relationship views: source then
        // destination per view, first-seen wins.
        // ONE pass over the (deterministic) relationship-view list.
        let rels = e.view_relationships(view);
        let mut elements: Vec<ElemRef<'a>> = Vec::new();
        for rv in &rels {
            let relationship = e.relationship(rv)?;
            for id in [&relationship.source, &relationship.destination] {
                let element = element_by_id(e.model(), id).ok_or(ExportError::ReferenceAbort(
                    "dangling relationship endpoint",
                ))?;
                if !elements.iter().any(|x| x.id() == element.id()) {
                    elements.push(element);
                }
            }
        }
        for element in &elements {
            Self::write_participant(e, view, *element, &mut w)?;
        }
        // ALWAYS written, even with no participants.
        w.write_line(None);

        for rv in &rels {
            let relationship = e.relationship(rv)?;
            let response = rv.response == Some(true);
            let (source, destination) = if response {
                (&relationship.destination, &relationship.source)
            } else {
                (&relationship.source, &relationship.destination)
            };
            let arrow = if response { "-->>" } else { "->>" };
            // Escaping a null description is the official null-pointer crash: a null
            // description aborts (no invented text).
            let description = rv
                .description
                .as_deref()
                .filter(|d| is_value(d))
                .or(relationship.description.as_deref())
                .ok_or(ExportError::ReferenceAbort(
                    "sequence message without a description (Java escapeSemicolon(null) NPE)",
                ))?;
            let technology = match relationship.technology.as_deref() {
                Some(technology) if is_value(technology) => format!("<br />[{technology}]"),
                _ => String::new(),
            };
            w.write_line(Some(&format!(
                "{source}{arrow}{destination}: {}{}",
                escape_semicolon(description),
                escape_semicolon(&technology)
            )));
        }
        Ok(self.create_diagram(e, view, w.to_string()))
    }

    fn create_diagram(&self, _e: &Engine, view: View<'_>, definition: String) -> Diagram {
        Diagram::new(view.key().to_string(), definition, "mmd".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidizr_core::configuration::Workspace;
    use oxidizr_core::docs::Documentation;
    use oxidizr_core::views::{AutomaticLayout, SystemLandscapeView, ViewBase, ViewContent};
    use oxidizr_core::{
        Id,
        model::{DeploymentNode, ModelItem, Person, Relationship, SoftwareSystem},
        properties::Properties,
        tags::Tags,
        views::{ColorScheme, ElementView, RelationshipView},
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
    /// relationship `5` ("Uses"), no automatic layout.
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

    fn exported(workspace: &Workspace) -> Vec<Diagram> {
        let mut engine = Engine::new(workspace, ColorScheme::Light);
        engine
            .export_all(&mut MermaidDialect::new())
            .expect("export")
    }

    #[test]
    fn lines_wraps_greedily_at_30_utf16_columns() {
        // `line.length() + word.length() + 1 < 30` — words of 15 chars
        // each: "aaaa… aaaa…" is 15+1+15 = 31, so the second word
        // always starts a new line.
        assert_eq!(
            lines("aaaaaaaaaaaaaaa aaaaaaaaaaaaaaa b"),
            "aaaaaaaaaaaaaaa<br />aaaaaaaaaaaaaaa b"
        );
        assert_eq!(lines("  padded   words  "), "padded words");
        assert_eq!(lines(""), "");
    }

    #[test]
    fn header_footer_and_default_direction() {
        let diagram = exported(&workspace()).pop().expect("one diagram");
        assert_eq!(diagram.file_extension(), "mmd");
        assert_eq!(
            diagram.definition(),
            "graph TB\n\
             \x20 linkStyle default fill:#ffffff\n\
             \n\
             \x20 subgraph diagram [\"System Landscape View\"]\n\
             \x20   style diagram fill:#ffffff,stroke:#ffffff\n\
             \n\
             \x20   1[\"<div style='font-weight: bold'>Alice</div><div style='font-size: 70%; margin-top: 0px'>[Person]</div>\"]\n\
             \x20   style 1 fill:#ffffff,stroke:#444444,color:#444444\n\
             \x20   2[\"<div style='font-weight: bold'>Big Bank</div><div style='font-size: 70%; margin-top: 0px'>[Software System]</div>\"]\n\
             \x20   style 2 fill:#ffffff,stroke:#444444,color:#444444\n\
             \n\
             \x20   1-. \"<div>Uses</div><div style='font-size: 70%'></div>\" .->2\n\
             \n\
             \x20 end"
        );
    }

    #[test]
    fn title_off_is_case_sensitive_and_renders_blank() {
        let mut ws = workspace();
        // `mermaid.title` = "TRUE" — case-SENSITIVE `equals` MISSES, so
        // the title leg renders the single-space placeholder.
        ws.views.system_landscape_views[0]
            .base
            .properties
            .insert("mermaid.title", "TRUE");
        let diagram = exported(&ws).pop().expect("one diagram");
        assert!(
            diagram.definition().contains("subgraph diagram [\" \"]"),
            "{}",
            diagram.definition()
        );
    }

    #[test]
    fn automatic_layout_drives_direction_null_direction_aborts() {
        let mut ws = workspace();
        ws.views.system_landscape_views[0].content.automatic_layout = Some(AutomaticLayout {
            rank_direction: Some(RankDirection::RightLeft),
            ..AutomaticLayout::default()
        });
        let diagram = exported(&ws).pop().expect("export");
        assert!(diagram.definition().starts_with("graph RL\n"));

        let mut ws = workspace();
        // A present layout with a null rank direction is the official
        // null-pointer crash.
        ws.views.system_landscape_views[0].content.automatic_layout = Some(AutomaticLayout {
            rank_direction: None,
            ..AutomaticLayout::default()
        });
        let err = Engine::new(&ws, ColorScheme::Light)
            .export_all(&mut MermaidDialect::new())
            .expect_err("abort");
        assert!(matches!(err, ExportError::ReferenceAbort(_)), "{err}");
    }

    #[test]
    fn element_shape_symbols_and_click_leg() {
        let mut ws = workspace();
        // `Person` → rounded box + url → `( … )` symbols and a `click` leg.
        ws.views
            .configuration
            .styles
            .element_styles
            .push(ElementStyle {
                tag: "Person".to_string(),
                shape: Some(Shape::RoundedBox),
                ..ElementStyle::default()
            });
        ws.model.people[0].item.url = Some("https://example.invalid/alice".into());
        let diagram = exported(&ws).pop().expect("export");
        let def = diagram.definition();
        assert!(
            def.contains("1(\"<div style='font-weight: bold'>Alice</div>"),
            "{def}"
        );
        assert!(
            def.contains("click 1 https://example.invalid/alice \"https://example.invalid/alice\""),
            "{def}"
        );
        // The system keeps the default `Box` symbols.
        assert!(
            def.contains("2[\"<div style='font-weight: bold'>Big Bank</div>"),
            "{def}"
        );
    }

    #[test]
    fn group_boundaries_cut_at_the_last_separator() {
        let mut ws = workspace();
        ws.model
            .properties
            .insert("structurizr.groupSeparator", "/");
        ws.model.people[0].group = Some("A/B".into());
        ws.model.software_systems[0].group = Some("A/B".into());
        let mut view = ws.views.system_landscape_views[0].clone();
        view.content.relationships.clear();
        ws.views.system_landscape_views[0] = view;
        let diagram = exported(&ws).pop().expect("export");
        let def = diagram.definition();
        // The engine expands `A/B` into the nested walk (`groups-nested`
        // SAME verified case): each boundary displays the part after ITS last
        // separator, and with no `Group:`/`Group` style declared the color
        // falls back to `#cccccc`.
        assert!(def.contains("subgraph group1 [\"A\"]\n"), "{def}");
        assert!(def.contains("subgraph group2 [\"B\"]\n"), "{def}");
        assert!(
            def.contains(
                "style group2 fill:#ffffff,stroke:#cccccc,color:#cccccc,stroke-dasharray:5"
            ),
            "{def}"
        );
    }

    #[test]
    fn sequence_document_is_a_standalone_diagram() {
        let mut ws = workspace();
        ws.model.relationships[0].description = Some("uses; now".into());
        ws.model.relationships[0].technology = Some("HTTP".into());
        let mut dv = DynamicView {
            base: ViewBase::new("dyn", 1),
            content: ViewContent::default(),
            element_id: None,
            deployment_environment: None,
            external_boundaries_visible: false,
        };
        // `mermaid.sequenceDiagram` compares case-insensitively — "TRUE" gates.
        dv.base.properties.insert("mermaid.sequenceDiagram", "TRUE");
        dv.content.elements = vec![ev("1"), ev("2")];
        dv.content.relationships = vec![rv("5")];
        ws.views.dynamic_views.push(dv);
        // Only the dynamic view exports (the landscape stays a `graph`).
        ws.views.system_landscape_views.clear();
        ws.model.people[0].item.description = Some("A person; two halves".into());
        // Person shape → `actor` (the theme would set it; pin it explicitly).
        ws.views
            .configuration
            .styles
            .element_styles
            .push(ElementStyle {
                tag: "Person".to_string(),
                shape: Some(Shape::Person),
                ..ElementStyle::default()
            });
        let diagram = exported(&ws).pop().expect("one diagram");
        assert_eq!(
            diagram.definition(),
            "sequenceDiagram\n\
             \n\
             \x20 actor 1 as Alice<br />[Person]\n\
             \x20 participant 2 as Big Bank<br />[Software System]\n\
             \n\
             \x20 1->>2: uses#59; now<br />[HTTP]"
        );
    }

    #[test]
    fn sequence_response_leg_swaps_and_reverses_the_arrow() {
        let mut ws = workspace();
        let mut dv = DynamicView {
            base: ViewBase::new("dyn", 1),
            content: ViewContent::default(),
            element_id: None,
            deployment_environment: None,
            external_boundaries_visible: false,
        };
        dv.base.properties.insert("mermaid.sequenceDiagram", "true");
        let mut response = rv("5");
        response.response = Some(true);
        dv.content.elements = vec![ev("1"), ev("2")];
        dv.content.relationships = vec![response];
        ws.views.system_landscape_views.clear();
        ws.views.dynamic_views.push(dv);
        let diagram = exported(&ws).pop().expect("one diagram");
        assert!(
            diagram.definition().ends_with("  2-->>1: Uses"),
            "{}",
            diagram.definition()
        );
    }

    #[test]
    fn sequence_null_description_aborts_like_the_java_npe() {
        let mut ws = workspace();
        ws.model.relationships[0].description = None;
        let mut dv = DynamicView {
            base: ViewBase::new("dyn", 1),
            content: ViewContent::default(),
            element_id: None,
            deployment_environment: None,
            external_boundaries_visible: false,
        };
        dv.base.properties.insert("mermaid.sequenceDiagram", "true");
        dv.content.elements = vec![ev("1"), ev("2")];
        dv.content.relationships = vec![rv("5")];
        ws.views.system_landscape_views.clear();
        ws.views.dynamic_views.push(dv);
        let err = Engine::new(&ws, ColorScheme::Light)
            .export_all(&mut MermaidDialect::new())
            .expect_err("Java escapeSemicolon(null) NPE");
        assert!(matches!(err, ExportError::ReferenceAbort(_)), "{err}");
    }

    #[test]
    fn deployment_node_endpoints_kill_the_edge() {
        let mut ws = workspace();
        // Element `2` becomes a DeploymentNode: the 1→2 edge disappears
        // (either endpoint is enough) while the workspace still
        // exports.
        let dn = DeploymentNode {
            item: item("2", "Server"),
            group: None,
            environment: "Default".into(),
            technology: None,
            instances: "1".into(),
            deployment_groups: Vec::new(),
            children: Vec::new(),
            infrastructure_nodes: Vec::new(),
            software_system_instances: Vec::new(),
            container_instances: Vec::new(),
            relationships: Vec::new(),
        };
        ws.model.software_systems.clear();
        ws.model.deployment_nodes = vec![dn];
        let diagram = exported(&ws).pop().expect("export");
        let def = diagram.definition();
        // The 1→2 edge is skipped; the node itself writes as an element
        // (the DN boundary leg only runs inside a DeploymentView).
        assert!(!def.contains(".-> 2"), "{def}");
        assert!(
            def.contains("2[\"<div style='font-weight: bold'>Server</div>"),
            "{def}"
        );
    }
}
