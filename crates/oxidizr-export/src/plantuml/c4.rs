//! The C4-PlantUML dialect — port of
//! `export/plantuml/C4PlantUMLExporter` (catalog
//! `agents/plan/build/EXPORT-C4-PLANTUML.md`, pinned `ad34036`).
//!
//! Every line format, gate and quirk mirrors the Java source: the ALWAYS
//! `<style>` root block with the P2-V5 `Styles.DEFAULT_*` constants, the
//! `<C4/…>` include accumulation (sequence: `C4_Sequence` only), the
//! tags-section `AddElementTag`/`AddRelTag`/`AddBoundaryTag` passes over
//! Java `HashMap` order (`oxidizr_json::javahash::entry_order` — portable,
//! unlike D8/D13), the boundary/group/element/relationship macros with the
//! quirk register (`, $borderThickness="1")` drop, boundary `#ffffff`
//! background + stroke-as-font quirk, quoted `_boundary` aliases, the
//! `group_<n>` counter, `(xN)` instance suffixes, response endpoint swap)
//! and the lifecycle deltas (`export(CustomView)` = `null` skip, legend
//! `null` — no `-key` files, animation gated off for dynamic views).

use oxidizr_core::properties::Properties;
use oxidizr_core::validate::ElementKind;
use oxidizr_core::views::{ElementStyle, LineStyle, RelationshipStyle, Shape, View};
use oxidizr_json::javahash::entry_order;

use crate::diagram::Diagram;
use crate::error::ExportError;
use crate::indent::IndentingWriter;
use crate::plantuml::engine::{Dialect, Engine, is_null_or_empty, is_true_ci, is_value};
use crate::plantuml::model::{ElemRef, element_by_id};
use crate::plantuml::structurizr::{
    StructurizrDialect, color_or_null, defaults, element_style_of, id_of, view_name,
};
use crate::plantuml::{
    C4PLANTUML_ELEMENT_PROPERTIES_PROPERTY, C4PLANTUML_LEGEND_PROPERTY,
    C4PLANTUML_RELATIONSHIP_PROPERTIES_PROPERTY, C4PLANTUML_SHADOW, C4PLANTUML_SPRITE,
    C4PLANTUML_STANDARD_LIBRARY_PROPERTY, C4PLANTUML_STEREOTYPES_PROPERTY,
    C4PLANTUML_TAGS_PROPERTY, DIAGRAM_DESCRIPTION_TAG, DIAGRAM_TITLE_TAG, ICON_SCALE,
    PLANTUML_ANIMATION_PROPERTY, PLANTUML_SEQUENCE_DIAGRAM_PROPERTY, is_supported_icon,
    java_cmp_str,
};

/// The non-`<style>`-block C4 dialect: the engine drives the iteration, the
/// dialect owns the per-diagram `groupId` counter (Java `groupId`, reset in
/// `writeHeader`).
#[derive(Debug, Default)]
pub struct C4PlantUmlDialect {
    group_id: u32,
}

/// `put` + append: insert `(key, value)` into the entry list (replace in
/// place on a duplicate key, push on a new one — Java `HashMap.put` keeps
/// the original insertion position).
fn put_entry<V>(entries: &mut Vec<(String, V)>, key: String, value: V) {
    if let Some(slot) = entries.iter_mut().find(|(k, _)| *k == key) {
        slot.1 = value;
    } else {
        entries.push((key, value));
    }
}

/// The Java `HashMap` iteration order of an already-distinct entry list.
fn java_order<T>(entries: &[(String, T)]) -> Vec<usize> {
    let keys: Vec<&str> = entries.iter().map(|(k, _)| k.as_str()).collect();
    entry_order(&keys)
}

/// `Border.toString().toLowerCase()` — the merged style always carries a
/// border (the synthesized default is `Solid`).
fn border_lower(border: Option<LineStyle>) -> &'static str {
    match border.unwrap_or(LineStyle::Solid) {
        LineStyle::Solid => "solid",
        LineStyle::Dashed => "dashed",
        LineStyle::Dotted => "dotted",
    }
}

/// `addProperties`: drop `structurizr.` keys; when any remain, the
/// `WithoutPropertyHeader()` marker then `AddProperty` per key in SORTED
/// order (the Java `stream().sorted()` — `String` natural order).
/// One Java color cascade: the `Group:{name}` style wins over the `Group`
/// style, null/empty values skipped; first valid hit wins, `None` keeps the
/// caller's current value (2R R-exp-14 — `start_group_boundary` spelled this
/// cascade four times as inline `to_string()` copies).
fn cascade_color<'s>(
    group: Option<&'s ElementStyle>,
    all: Option<&'s ElementStyle>,
    pick: impl Fn(&'s ElementStyle) -> Option<&'s str>,
) -> Option<&'s str> {
    group
        .and_then(|s| pick(s).filter(|c| !is_null_or_empty(Some(c))))
        .or_else(|| all.and_then(|s| pick(s).filter(|c| !is_null_or_empty(Some(c)))))
}

fn add_properties(w: &mut IndentingWriter, properties: &Properties) {
    let mut entries: Vec<(&str, &str)> = properties
        .iter()
        .filter(|(key, _)| !key.starts_with("structurizr."))
        .collect();
    if entries.is_empty() {
        return;
    }
    entries.sort_by(|a, b| java_cmp_str(a.0, b.0));
    w.write_line(Some("WithoutPropertyHeader()"));
    for (key, value) in entries {
        w.write_line(Some(&format!("AddProperty(\"{key}\",\"{value}\")")));
    }
}

impl C4PlantUmlDialect {
    /// A fresh dialect (the group-id counter starts at 0; the header resets
    /// it per diagram exactly like the Java field).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn include_tags<'a>(e: &Engine<'a>, view: View<'a>) -> bool {
        is_true_ci(e.property(view.base(), C4PLANTUML_TAGS_PROPERTY, "false"))
    }

    fn element_properties<'a>(e: &Engine<'a>, view: View<'a>) -> bool {
        is_true_ci(e.property(view.base(), C4PLANTUML_ELEMENT_PROPERTIES_PROPERTY, "false"))
    }

    fn relationship_properties<'a>(e: &Engine<'a>, view: View<'a>) -> bool {
        is_true_ci(e.property(
            view.base(),
            C4PLANTUML_RELATIONSHIP_PROPERTIES_PROPERTY,
            "false",
        ))
    }

    /// `tagsOf(view, element)` — the MERGED element style tag, the leading
    /// `Element,` dropped (only the first occurrence — quirk §10.7), or
    /// empty when `c4plantuml.tags` is off.
    fn element_tags_of<'a>(
        e: &Engine<'a>,
        view: View<'a>,
        element: ElemRef<'a>,
    ) -> Result<String, ExportError> {
        if Self::include_tags(e, view) {
            Ok(element_style_of(e, element)?
                .tag
                .replacen("Element,", "", 1))
        } else {
            Ok(String::new())
        }
    }

    /// `tagsOf(view, relationship)` — the merged relationship style tag,
    /// the leading `Relationship,` dropped.
    fn relationship_tags_of<'a>(
        e: &Engine<'a>,
        view: View<'a>,
        relationship: &oxidizr_core::model::Relationship,
    ) -> String {
        if Self::include_tags(e, view) {
            e.configuration()
                .styles
                .relationship_style(relationship, e.model(), e.color_scheme())
                .tag
                .replacen("Relationship,", "", 1)
        } else {
            String::new()
        }
    }

    /// `usePlantUMLStandardLibrary` — the `<C4/NAME>` vs raw-URL include
    /// form.
    fn include_line<'a>(e: &Engine<'a>, view: View<'a>, name: &str) -> String {
        if is_true_ci(e.property(view.base(), C4PLANTUML_STANDARD_LIBRARY_PROPERTY, "true")) {
            format!("!include <C4/{name}>")
        } else {
            format!(
                "!include https://raw.githubusercontent.com/plantuml-stdlib/C4-PlantUML/master/{name}.puml"
            )
        }
    }

    /// The boundary elements of the tags section (`getBoundaryElements`):
    /// container views collect the system boundaries, component views the
    /// container boundaries, dynamic views by element-scope kind — and the
    /// whole pass is skipped on the sequence path.
    fn boundary_elements<'a>(&self, e: &Engine<'a>, view: View<'a>) -> Vec<ElemRef<'a>> {
        let mut boundaries: Vec<ElemRef<'a>> = Vec::new();
        if self.render_as_sequence_diagram(e, view) {
            return boundaries;
        }
        match view {
            View::Container(_) => boundaries.extend(e.get_boundary_systems(view)),
            View::Component(_) => boundaries.extend(e.get_boundary_containers(view)),
            View::Dynamic(dv) => {
                let scope = dv
                    .element_id
                    .as_ref()
                    .and_then(|id| element_by_id(e.model(), id));
                match scope.map(ElemRef::kind) {
                    Some(ElementKind::SoftwareSystem) => {
                        boundaries.extend(e.get_boundary_systems(view));
                    }
                    Some(ElementKind::Container) => {
                        boundaries.extend(e.get_boundary_containers(view));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        boundaries
    }

    /// The `c4plantuml.tags` section: the three Java `HashMap<String, Style>`
    /// collections (elements, relationships, boundaries — boundaries only on
    /// the non-sequence path) rendered in `HashMap` iteration order.
    fn write_tags<'a>(
        &self,
        e: &Engine<'a>,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let mut element_styles: Vec<(String, oxidizr_core::views::ElementStyle)> = Vec::new();
        let mut relationship_styles: Vec<(String, RelationshipStyle)> = Vec::new();
        let mut boundary_styles: Vec<(String, oxidizr_core::views::ElementStyle)> = Vec::new();

        for element in e.view_elements(view) {
            let style = element_style_of(e, element)?;
            put_entry(&mut element_styles, style.tag.clone(), style);
        }
        for relationship_view in e.view_relationships(view) {
            let relationship = e.relationship(relationship_view)?;
            let style = e.configuration().styles.relationship_style(
                relationship,
                e.model(),
                e.color_scheme(),
            );
            put_entry(&mut relationship_styles, style.tag.clone(), style);
        }

        for element in self.boundary_elements(e, view) {
            let style = element_style_of(e, element)?;
            put_entry(&mut boundary_styles, style.tag.clone(), style);
        }

        if !element_styles.is_empty() {
            for index in java_order(&element_styles) {
                let (tag_list, style) = &element_styles[index];
                let tag = tag_list.replacen("Element,", "", 1);
                let mut sprite = String::new();
                if is_supported_icon(style.icon.as_deref()) {
                    // The D12 pin: the reference `calculateIconScale` needs
                    // a network fetch — this port renders the constant.
                    sprite = format!(
                        "img:{}{{scale={}}}",
                        style.icon.clone().unwrap_or_default(),
                        ICON_SCALE
                    );
                }
                let sprite = style
                    .properties
                    .get(C4PLANTUML_SPRITE)
                    .unwrap_or(sprite.as_str());
                let thickness = style.stroke_width.unwrap_or(1);
                let line = format!(
                    "AddElementTag(\"{tag}\", $bgColor=\"{}\", $borderColor=\"{}\", $fontColor=\"{}\", $sprite=\"{sprite}\", $shadowing=\"{}\", $borderStyle=\"{}\", $borderThickness=\"{thickness}\")",
                    color_or_null(style.background.as_deref()),
                    color_or_null(style.stroke.as_deref()),
                    color_or_null(style.color.as_deref()),
                    style.properties.get(C4PLANTUML_SHADOW).unwrap_or(""),
                    border_lower(style.border),
                );
                w.write_line(Some(&line.replace(", $borderThickness=\"1\")", ")")));
            }
            w.write_line(None);
        }

        if !relationship_styles.is_empty() {
            for index in java_order(&relationship_styles) {
                let (tag_list, style) = &relationship_styles[index];
                let tag = tag_list.replacen("Relationship,", "", 1);
                let line_style = match style.style {
                    Some(LineStyle::Dashed) => "DashedLine()",
                    Some(LineStyle::Dotted) => "DottedLine()",
                    _ => "\"\"",
                };
                let color = color_or_null(style.color.as_deref());
                w.write_line(Some(&format!(
                    "AddRelTag(\"{tag}\", $textColor=\"{color}\", $lineColor=\"{color}\", $lineStyle = {line_style})"
                )));
            }
            w.write_line(None);
        }

        if !boundary_styles.is_empty() {
            for index in java_order(&boundary_styles) {
                let (tag_list, style) = &boundary_styles[index];
                let tag = tag_list.replacen("Element,", "", 1);
                let thickness = style.stroke_width.unwrap_or(1);
                let stroke = color_or_null(style.stroke.as_deref());
                let line = format!(
                    "AddBoundaryTag(\"{tag}\", $bgColor=\"#ffffff\", $borderColor=\"{stroke}\", $fontColor=\"{stroke}\", $shadowing=\"{}\", $borderStyle=\"{}\", $borderThickness=\"{thickness}\")",
                    style.properties.get(C4PLANTUML_SHADOW).unwrap_or(""),
                    border_lower(style.border),
                );
                w.write_line(Some(&line.replace(", $borderThickness=\"1\")", ")")));
            }
            w.write_line(None);
        }
        Ok(())
    }
}

impl Dialect for C4PlantUmlDialect {
    fn write_header<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        // `AbstractPlantUMLExporter.writeHeader` (the `super` legs).
        w.write_line(Some("@startuml"));
        if StructurizrDialect::include_title(e, view) {
            let styles = &e.configuration().styles;
            let title_style = styles.find_element_style(DIAGRAM_TITLE_TAG, e.color_scheme());
            let description_style =
                styles.find_element_style(DIAGRAM_DESCRIPTION_TAG, e.color_scheme());
            let title = match view.base().title.as_deref() {
                Some(title) if is_value(title) => title.to_string(),
                _ => view_name(e, view)?,
            };
            let description = view.base().description.as_deref().filter(|d| is_value(d));
            match description {
                None => w.write_line(Some(&format!(
                    "title <size:{}>{}</size>",
                    StructurizrDialect::font_of(title_style.as_ref()),
                    title
                ))),
                Some(description) => w.write_line(Some(&format!(
                    "title <size:{}>{}</size>\\n<size:{}>{}</size>",
                    StructurizrDialect::font_of(title_style.as_ref()),
                    title,
                    StructurizrDialect::font_of(description_style.as_ref()),
                    description
                ))),
            }
        }
        w.write_line(None);
        w.write_line(Some("set separator none"));

        // Java `groupId = 0` right after the `super` header.
        self.group_id = 0;

        let sequence = self.render_as_sequence_diagram(e, view);
        if !sequence {
            // The direction line is ALWAYS written (unlike the Structurizr
            // dialect, no ranksep/nodesep legs follow).
            w.write_line(Some(
                match crate::plantuml::structurizr::automatic_layout(view) {
                    Some(layout)
                        if matches!(
                            layout.rank_direction,
                            Some(oxidizr_core::views::RankDirection::LeftRight)
                        ) =>
                    {
                        "left to right direction"
                    }
                    _ => "top to bottom direction",
                },
            ));
        }

        StructurizrDialect::write_skin_params(e, view, w)?;

        // The ALWAYS-`<style>` root block (P2-V5: the exporter field —
        // `Default` renders the LIGHT constants).
        let (background, color) = defaults(e.color_scheme());
        w.write_line(Some("<style>"));
        w.indent();
        w.write_line(Some("root {"));
        w.indent();
        w.write_line(Some(&format!("BackgroundColor: {background}")));
        w.write_line(Some(&format!("FontColor: {color}")));
        w.outdent();
        w.write_line(Some("}"));
        w.outdent();
        w.write_line(Some("</style>"));
        w.write_line(None);

        if sequence {
            w.write_line(Some(&Self::include_line(e, view, "C4_Sequence")));
        } else {
            w.write_line(Some(&Self::include_line(e, view, "C4")));
            w.write_line(Some(&Self::include_line(e, view, "C4_Context")));
            let elements = e.view_elements(view);
            if elements.iter().any(|element| {
                matches!(
                    element.kind(),
                    ElementKind::Container | ElementKind::ContainerInstance
                )
            }) {
                w.write_line(Some(&Self::include_line(e, view, "C4_Container")));
            }
            if elements
                .iter()
                .any(|element| element.kind() == ElementKind::Component)
            {
                w.write_line(Some(&Self::include_line(e, view, "C4_Component")));
            }
            if matches!(view, View::Deployment(_)) {
                w.write_line(Some(&Self::include_line(e, view, "C4_Deployment")));
            }
        }
        w.write_line(None);

        StructurizrDialect::write_includes(e, view, w);

        if Self::include_tags(e, view) {
            self.write_tags(e, view, w)?;
        }
        Ok(())
    }

    fn write_footer<'a>(&mut self, e: &Engine<'a>, view: View<'a>, w: &mut IndentingWriter) {
        w.write_line(Some(&format!(
            "SHOW_LEGEND({})",
            is_true_ci(e.property(view.base(), C4PLANTUML_LEGEND_PROPERTY, "true"))
        )));
        w.write_line(Some(
            if is_true_ci(e.property(view.base(), C4PLANTUML_STEREOTYPES_PROPERTY, "false")) {
                "show stereotypes"
            } else {
                "hide stereotypes"
            },
        ));
        w.write_line(Some("@enduml"));
    }

    fn write_element<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        element: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        if element.kind() == ElementKind::CustomElement {
            return Ok(());
        }

        let style = element_style_of(e, element)?;
        let id = id_of(e, element)?;

        let mut url = element.url().unwrap_or("");

        if Self::element_properties(e, view) {
            add_properties(w, &element.item().properties);
        }

        // `StaticStructureElementInstance` macros describe the TARGET
        // element (its name/description/technology/url); `$tags` stay the
        // INSTANCE's resolved style (`element`).
        let is_instance = matches!(
            element.kind(),
            ElementKind::SoftwareSystemInstance | ElementKind::ContainerInstance
        );
        let written = if is_instance {
            let target = element
                .target(e.model())
                .ok_or(ExportError::ReferenceAbort(
                    "instance references a missing element (Java getElement() NPE)",
                ))?;
            if url.is_empty() {
                url = target.url().unwrap_or("");
            }
            target
        } else {
            element
        };

        let name = written.name();
        let description = written.description().unwrap_or("");
        let tags = Self::element_tags_of(e, view, element)?;

        match written.kind() {
            ElementKind::Person => w.write_line(Some(&format!(
                "Person({id}, \"{name}\", $descr=\"{description}\", $tags=\"{tags}\", $link=\"{url}\")"
            ))),
            ElementKind::SoftwareSystem => w.write_line(Some(&format!(
                "System({id}, \"{name}\", $descr=\"{description}\", $tags=\"{tags}\", $link=\"{url}\")"
            ))),
            ElementKind::Container | ElementKind::Component => {
                let shape = match style.shape {
                    Some(Shape::Cylinder) => "Db",
                    Some(Shape::Pipe) => "Queue",
                    _ => "",
                };
                let technology = written.technology().unwrap_or("");
                let macro_name = if written.kind() == ElementKind::Container {
                    "Container"
                } else {
                    "Component"
                };
                w.write_line(Some(&format!(
                    "{macro_name}{shape}({id}, \"{name}\", $techn=\"{technology}\", $descr=\"{description}\", $tags=\"{tags}\", $link=\"{url}\")"
                )));
            }
            ElementKind::InfrastructureNode => {
                let technology = written.technology().unwrap_or("");
                w.write_line(Some(&format!(
                    "Deployment_Node({id}, \"{name}\", $type=\"{technology}\", $descr=\"{description}\", $tags=\"{tags}\", $link=\"{url}\")"
                )));
            }
            _ => {}
        }

        if !e.is_visible(view, element)? {
            w.write_line(Some(&format!("hide {id}")));
        }
        Ok(())
    }

    fn write_relationship<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        relationship_view: &'a oxidizr_core::views::RelationshipView,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let relationship = e.relationship(relationship_view)?;
        let dangling =
            || ExportError::ReferenceAbort("relationship endpoint does not resolve (Java NPE)");
        let mut source = element_by_id(e.model(), &relationship.source).ok_or_else(dangling)?;
        let mut destination =
            element_by_id(e.model(), &relationship.destination).ok_or_else(dangling)?;
        if source.kind() == ElementKind::CustomElement
            || destination.kind() == ElementKind::CustomElement
        {
            return Ok(());
        }

        if Self::relationship_properties(e, view) {
            add_properties(w, &relationship.properties);
        }

        // `isResponse` swaps the `Rel` endpoints (dynamic views).
        if relationship_view.response == Some(true) {
            std::mem::swap(&mut source, &mut destination);
        }

        let mut description = String::new();
        if let Some(order) = &relationship_view.order {
            if !is_null_or_empty(Some(order)) {
                description = format!("{order}: ");
            }
        }
        description.push_str(
            relationship_view
                .description
                .as_deref()
                .filter(|d| is_value(d))
                .or_else(|| relationship.description.as_deref().filter(|d| is_value(d)))
                .unwrap_or(""),
        );

        // Java: `technology = getTechnology(); if (isNullOrEmpty) ""` —
        // which is exactly `unwrap_or("")`.
        let technology = relationship.technology.as_deref().unwrap_or("");
        let url = relationship.url.as_deref().unwrap_or("");
        let tags = Self::relationship_tags_of(e, view, relationship);
        let source_id = id_of(e, source)?;
        let destination_id = id_of(e, destination)?;
        w.write_line(Some(&format!(
            "Rel({source_id}, {destination_id}, \"{description}\", $techn=\"{technology}\", $tags=\"{tags}\", $link=\"{url}\")"
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
        let group_name = StructurizrDialect::group_display_name(
            group,
            e.model().properties.get("structurizr.groupSeparator"),
        )?;

        let styles = &e.configuration().styles;
        let scheme = e.color_scheme();
        let group_style = styles.find_element_style(&format!("Group:{group}"), scheme);
        let all_style = styles.find_element_style("Group", scheme);
        let (g, a) = (group_style.as_ref(), all_style.as_ref());

        // `#cccccc` default; the Group style COLOR, then the STROKE
        // overrides it (two independent Java cascades), then the width.
        // 2R R-exp-14: one helper call per Java cascade — the old shape
        // spelled the group-over-all null/empty filter four times inline,
        // copying a String each leg.
        let mut color = "#cccccc";
        if let Some(c) = cascade_color(g, a, |s| s.color.as_deref()) {
            color = c;
        }
        if let Some(c) = cascade_color(g, a, |s| s.stroke.as_deref()) {
            color = c;
        }
        let thickness = g
            .and_then(|s| s.stroke_width)
            .or_else(|| a.and_then(|s| s.stroke_width))
            .unwrap_or(1);

        let line = format!(
            "AddBoundaryTag(\"{group}\", $borderColor=\"{color}\", $fontColor=\"{color}\", $borderStyle=\"dashed\", $borderThickness=\"{thickness}\")"
        );
        w.write_line(Some(&line.replace(", $borderThickness=\"1\")", ")")));
        w.write_line(Some(&format!(
            "Boundary(group_{}, \"{group_name}\", $tags=\"{group}\") {{",
            self.group_id
        )));
        w.indent();
        Ok(())
    }

    fn end_group_boundary<'a>(
        &mut self,
        _e: &Engine<'a>,
        _view: View<'a>,
        w: &mut IndentingWriter,
    ) {
        w.outdent();
        w.write_line(Some("}"));
        w.write_line(None);
    }

    fn start_software_system_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        system: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) {
        // The abort legs of `idOf`/style resolution are unreachable for a
        // boundary element the engine resolved from the model.
        let id = id_of(e, system).unwrap_or_default();
        let tags = if Self::include_tags(e, view) {
            e.configuration()
                .styles
                .element_style(&system.tags_csv(), e.color_scheme())
                .tag
                .replacen("Element,", "", 1)
        } else {
            String::new()
        };
        w.write_line(Some(&format!(
            "System_Boundary(\"{id}_boundary\", \"{}\", $tags=\"{tags}\") {{",
            system.name()
        )));
        w.indent();
    }

    fn end_software_system_boundary<'a>(
        &mut self,
        _e: &Engine<'a>,
        _view: View<'a>,
        w: &mut IndentingWriter,
    ) {
        w.outdent();
        w.write_line(Some("}"));
        w.write_line(None);
    }

    fn start_container_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        container: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) {
        // The abort legs of `idOf`/style resolution are unreachable for a
        // boundary element the engine resolved from the model (2R R-exp-15
        // cite, same idiom as the system leg above: behavior is the
        // contract, the `unwrap_or_default` stays).
        let id = id_of(e, container).unwrap_or_default();
        let tags = if Self::include_tags(e, view) {
            e.configuration()
                .styles
                .element_style(&container.tags_csv(), e.color_scheme())
                .tag
                .replacen("Element,", "", 1)
        } else {
            String::new()
        };
        w.write_line(Some(&format!(
            "Container_Boundary(\"{id}_boundary\", \"{}\", $tags=\"{tags}\") {{",
            container.name()
        )));
        w.indent();
    }

    fn end_container_boundary<'a>(
        &mut self,
        _e: &Engine<'a>,
        _view: View<'a>,
        w: &mut IndentingWriter,
    ) {
        w.outdent();
        w.write_line(Some("}"));
        w.write_line(None);
    }

    fn start_deployment_node_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        node: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let url = node.url().unwrap_or("");

        if Self::element_properties(e, view) {
            add_properties(w, &node.item().properties);
        }

        let technology = node.technology().unwrap_or("");
        let description = node.description().unwrap_or("");
        let instances = node.instances().unwrap_or("1");
        let name = if instances == "1" {
            node.name().to_string()
        } else {
            format!("{} (x{instances})", node.name())
        };
        let id = id_of(e, node)?;
        let tags = Self::element_tags_of(e, view, node)?;

        w.write_line(Some(&format!(
            "Deployment_Node({id}, \"{name}\", $type=\"{technology}\", $descr=\"{description}\", $tags=\"{tags}\", $link=\"{url}\") {{"
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

    /// The sequence-path boundary box (`box "Name\n<size:12>…</size>"`).
    fn start_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        _view: View<'a>,
        element: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) {
        w.write_line(Some(&format!(
            "box \"{}\\n<size:12>{}</size>\"",
            element.name(),
            e.type_of(element, true)
        )));
        w.indent();
    }

    fn end_boundary<'a>(&mut self, _e: &Engine<'a>, _view: View<'a>, w: &mut IndentingWriter) {
        w.outdent();
        w.write_line(Some("end box"));
    }

    fn is_animation_supported<'a>(&self, e: &Engine<'a>, view: View<'a>) -> bool {
        !matches!(view, View::Dynamic(_))
            && is_true_ci(e.property(view.base(), PLANTUML_ANIMATION_PROPERTY, "false"))
    }

    fn render_as_sequence_diagram<'a>(&self, e: &Engine<'a>, view: View<'a>) -> bool {
        matches!(view, View::Dynamic(_))
            && is_true_ci(e.property(view.base(), PLANTUML_SEQUENCE_DIAGRAM_PROPERTY, "false"))
    }

    /// Java `export(CustomView)` returns `null` — the workspace loop skips
    /// the view (witness: `custom-view-animation` writes NO files).
    fn exports_custom_views(&self) -> bool {
        false
    }

    fn create_diagram(&self, _e: &Engine, view: View<'_>, definition: String) -> Diagram {
        Diagram::new(view.key().to_string(), definition, "puml".to_string())
    }
}
