//! The `websequencediagrams` export — the official WSD exporter port.
//!
//! The official exporter emits ONLY `CustomView`s (the base
//! body export is not overridden) and
//! `DynamicView`s (an override with its own walk); the static-structure and
//! deployment overloads return `null`, so the [`export_all`] loop here skips
//! every other view kind. `renderAsSequenceDiagram` has no property gate:
//! the dynamic override IS the sequence form, so the dialect claims it via
//! the [`Dialect::sequence_document`] route (the Mermaid precedent).
//! Frames never (base `isAnimationSupported` is `false`); legend never
//! (the base legend is null — verified on `custom-view-animation`);
//! file extension `wsd` (`WebSequenceDiagramsDiagram.getFileExtension`).
//!
//! Byte rules (verified, `WebSequenceDiagramsExporter` +
//! `AbstractDiagramExporter` + `IndentingWriter`):
//!
//! - title and participant lines carry the LITERAL two-character `\n`
//!   (the official tooling format strings spell `\\n`); a real newline in a view
//!   description is escaped the same way by `writeLine` — our
//!   [`IndentingWriter::write_line`] escapes identically.
//! - `writeHeader` ends with a blank line; the dynamic walk writes a second
//!   blank after the participants UNCONDITIONALLY (zero participants still
//!   get it), while the custom-view base body guards it on a non-empty
//!   element list.
//! - `writeRelationships` (base) writes a trailing blank line when the view
//!   has relationships; the render strips exactly ONE trailing newline, so
//!   a relationship-bearing diagram's file ends with a single `\n`.
//! - a relationship with no view override and no model description prints
//!   EMPTY (`A->B: ` with the trailing space) — the official tooling, not a literal
//!   `null`.

use oxidizr_core::Workspace;
use oxidizr_core::tags::InteractionStyle;
use oxidizr_core::validate::ElementKind;
use oxidizr_core::views::{ColorScheme, DynamicView, RelationshipView, View};

use crate::diagram::Diagram;
use crate::error::ExportError;
use crate::indent::IndentingWriter;
use crate::plantuml::engine::{Dialect, Engine, is_null_or_empty, is_value};
use crate::plantuml::model::{ElemRef, element_by_id};
use crate::plantuml::structurizr::view_name;

/// The WSD dialect — stateless (no color scheme, no group counter).
#[derive(Debug, Default)]
pub struct WsdDialect;

impl WsdDialect {
    /// A fresh dialect.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// `Terminology.findTerminology` — `views.configuration.terminology`
    /// defaults with overrides; `StaticStructureElementInstance` unwraps to
    /// its element first; a `CustomElement` reports its `metadata`, never a
    /// configured term (the official chain has no custom-element branch).
    fn find_terminology<'a>(e: &Engine<'a>, element: ElemRef<'a>) -> Result<String, ExportError> {
        let resolved = if matches!(
            element.kind(),
            ElementKind::SoftwareSystemInstance | ElementKind::ContainerInstance
        ) {
            element
                .target(e.model())
                .ok_or(ExportError::ReferenceAbort(
                    "instance element does not resolve (Java unknown-type throw)",
                ))?
        } else {
            element
        };
        let t = &e.configuration().terminology;
        let chosen = |override_: &Option<String>, default: &str| -> String {
            match override_.as_deref() {
                Some(s) if is_value(s) => s.to_string(),
                _ => default.to_string(),
            }
        };
        Ok(match resolved.kind() {
            ElementKind::Person => chosen(&t.person, "Person"),
            ElementKind::SoftwareSystem => chosen(&t.software_system, "Software System"),
            ElementKind::Container => chosen(&t.container, "Container"),
            ElementKind::Component => chosen(&t.component, "Component"),
            ElementKind::DeploymentNode => chosen(&t.deployment_node, "Deployment Node"),
            ElementKind::InfrastructureNode => {
                chosen(&t.infrastructure_node, "Infrastructure Node")
            }
            ElementKind::CustomElement => match resolved.metadata() {
                Some(m) if is_value(m) => m.to_string(),
                _ => "Element".to_string(),
            },
            ElementKind::SoftwareSystemInstance | ElementKind::ContainerInstance => {
                return Err(ExportError::ReferenceAbort(
                    "instance element does not resolve (Java unknown-type throw)",
                ));
            }
        })
    }

    /// The dynamic-view document — the WSD `export(DynamicView)` override:
    /// header, participants from the relationship endpoints in dynamic order
    /// (source then destination, first-seen wins), the unconditional blank,
    /// then the base `writeRelationships` (order as stored, trailing blank
    /// when non-empty) and the no-op footer.
    fn sequence_diagram<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
    ) -> Result<Diagram, ExportError> {
        let mut w = IndentingWriter::new();
        self.write_header(e, view, &mut w)?;

        // LinkedHashSet over the relationship views: source then
        // destination per view, first-seen wins.
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
            self.write_element(e, view, *element, &mut w)?;
        }

        // Unconditional — even a participant-less view writes this blank.
        w.write_line(None);

        // `writeRelationships` (base): the dynamic view keeps the stored
        // order (no relationship-id sort).
        for rv in &rels {
            self.write_relationship(e, view, rv, &mut w)?;
        }
        if !rels.is_empty() {
            w.write_line(None);
        }

        // `writeFooter` is the no-op override; `createDiagram` the `wsd` ext.
        Ok(self.create_diagram(e, view, w.to_string()))
    }
}

impl Dialect for WsdDialect {
    /// `writeHeader` — `title <name>`, the description appended after a
    /// (literal-escaped) newline when the view has one; then the blank line.
    fn write_header<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let name = view_name(e, view)?;
        match view.base().description.as_deref() {
            Some(desc) if is_value(desc) => {
                w.write_line(Some(&format!("title {name}\n{desc}")));
            }
            _ => w.write_line(Some(&format!("title {name}"))),
        }
        w.write_line(None);
        Ok(())
    }

    /// `writeFooter` — the no-op override.
    fn write_footer<'a>(&mut self, _e: &Engine<'a>, _view: View<'a>, _w: &mut IndentingWriter) {}

    /// `writeElement` — `actor` for a `Person`, `participant` for everything
    /// else; the literal `\n` joins the stereotype to the `<name> as <name>`
    /// alias.
    fn write_element<'a>(
        &mut self,
        e: &Engine<'a>,
        _view: View<'a>,
        element: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let terminology = Self::find_terminology(e, element)?;
        let name = element.name();
        let verb = if matches!(element, ElemRef::Person(_)) {
            "actor"
        } else {
            "participant"
        };
        w.write_line(Some(&format!(
            "{verb} <<{terminology}>>\\n{name} as {name}"
        )));
        Ok(())
    }

    /// `writeRelationship` — `src->dst: desc` (`->>` asynchronous;
    /// a response swaps the endpoints and uses `-->`/`-->>`); the view
    /// override falls back to the model description, both absent to the
    /// EMPTY string (verified — not a literal `null` text).
    fn write_relationship<'a>(
        &mut self,
        e: &Engine<'a>,
        _view: View<'a>,
        relationship_view: &'a RelationshipView,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let relationship = e.relationship(relationship_view)?;
        let asynchronous = relationship.interaction_style == Some(InteractionStyle::Asynchronous);
        let mut source = &relationship.source;
        let mut destination = &relationship.destination;
        let mut arrow = if asynchronous { "->>" } else { "->" };
        if relationship_view.response == Some(true) {
            source = &relationship.destination;
            destination = &relationship.source;
            arrow = if asynchronous { "-->>" } else { "-->" };
        }
        let mut description = relationship_view.description.as_deref();
        if is_null_or_empty(description) {
            description = relationship.description.as_deref();
        }
        let source = element_by_id(e.model(), source).ok_or(ExportError::ReferenceAbort(
            "dangling relationship endpoint",
        ))?;
        let destination = element_by_id(e.model(), destination).ok_or(
            ExportError::ReferenceAbort("dangling relationship endpoint"),
        )?;
        w.write_line(Some(&format!(
            "{}{}{}: {}",
            source.name(),
            arrow,
            destination.name(),
            description.unwrap_or_default()
        )));
        Ok(())
    }

    // Every boundary hook is the empty override: the definitions carry no
    // group/system/container/deployment framing.
    fn start_group_boundary<'a>(
        &mut self,
        _e: &Engine<'a>,
        _view: View<'a>,
        _group: &str,
        _w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        Ok(())
    }
    fn end_group_boundary<'a>(
        &mut self,
        _e: &Engine<'a>,
        _view: View<'a>,
        _w: &mut IndentingWriter,
    ) {
    }
    fn start_software_system_boundary<'a>(
        &mut self,
        _e: &Engine<'a>,
        _view: View<'a>,
        _system: ElemRef<'a>,
        _w: &mut IndentingWriter,
    ) {
    }
    fn end_software_system_boundary<'a>(
        &mut self,
        _e: &Engine<'a>,
        _view: View<'a>,
        _w: &mut IndentingWriter,
    ) {
    }
    fn start_container_boundary<'a>(
        &mut self,
        _e: &Engine<'a>,
        _view: View<'a>,
        _container: ElemRef<'a>,
        _w: &mut IndentingWriter,
    ) {
    }
    fn end_container_boundary<'a>(
        &mut self,
        _e: &Engine<'a>,
        _view: View<'a>,
        _w: &mut IndentingWriter,
    ) {
    }
    fn start_deployment_node_boundary<'a>(
        &mut self,
        _e: &Engine<'a>,
        _view: View<'a>,
        _node: ElemRef<'a>,
        _w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        Ok(())
    }
    fn end_deployment_node_boundary<'a>(
        &mut self,
        _e: &Engine<'a>,
        _view: View<'a>,
        _w: &mut IndentingWriter,
    ) {
    }
    fn start_boundary<'a>(
        &mut self,
        _e: &Engine<'a>,
        _view: View<'a>,
        _element: ElemRef<'a>,
        _w: &mut IndentingWriter,
    ) {
    }
    fn end_boundary<'a>(&mut self, _e: &Engine<'a>, _view: View<'a>, _w: &mut IndentingWriter) {}

    /// The WSD dynamic override IS the sequence form (no property gate).
    fn render_as_sequence_diagram<'a>(&self, _e: &Engine<'a>, _view: View<'a>) -> bool {
        true
    }

    /// The walk above replaces the engine's `PlantUML` `box` route entirely.
    fn owns_sequence_document(&self) -> bool {
        true
    }

    fn sequence_document<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        _dv: &'a DynamicView,
    ) -> Result<Diagram, ExportError> {
        self.sequence_diagram(e, view)
    }

    /// `createDiagram` — the `wsd` file extension.
    fn create_diagram(&self, _e: &Engine, view: View<'_>, definition: String) -> Diagram {
        Diagram::new(view.key().to_string(), definition, "wsd".to_string())
    }
}

/// `WebSequenceDiagramsExporter.export(workspace)` — the base
/// `AbstractDiagramExporter` loop over the view kinds the exporter does not
/// null out: custom views (base body) then dynamic views (the sequence
/// override), each in stored order; every other kind returns `null` there
/// and is skipped here.
///
/// # Errors
///
/// [`ExportError::ReferenceAbort`] when a view aborts the export (the official tooling
/// cast/null-pointer crash paths of  — dangling endpoints, an unresolved view scope).
pub fn export_all(workspace: &Workspace) -> Result<Vec<Diagram>, ExportError> {
    // The exporter has no color-scheme constructor and never consults the
    // engine for one; the scheme is inert here.
    let mut engine = Engine::new(workspace, ColorScheme::Light);
    let mut dialect = WsdDialect::new();
    let mut diagrams = Vec::new();
    for view in workspace.views.all_views() {
        if matches!(view, View::Custom(_) | View::Dynamic(_)) {
            diagrams.push(engine.export_view(&mut dialect, view)?);
        }
    }
    Ok(diagrams)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidizr_core::Id;
    use oxidizr_core::docs::Documentation;
    use oxidizr_core::model::{ModelItem, Person, Relationship, SoftwareSystem};
    use oxidizr_core::properties::Properties;
    use oxidizr_core::tags::Tags;
    use oxidizr_core::views::{
        CustomView, ElementView, SystemLandscapeView, Terminology, ViewBase, ViewContent,
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

    fn custom_element(id: &str, name: &str) -> oxidizr_core::model::CustomElement {
        oxidizr_core::model::CustomElement {
            item: item(id, name),
            group: None,
            metadata: None,
            relationships: Vec::new(),
        }
    }

    fn rel(
        id: &str,
        source: &str,
        destination: &str,
        description: Option<&str>,
        asynchronous: bool,
    ) -> Relationship {
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
            interaction_style: asynchronous.then_some(InteractionStyle::Asynchronous),
            linked_relationship_id: None,
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

    /// Person `1` (Alice) → system `2` (Big Bank) via relationship `5`
    /// ("Uses"); one dynamic view carrying it.
    fn workspace() -> Workspace {
        let mut workspace = Workspace::default();
        workspace.model.people = vec![person("1", "Alice")];
        workspace.model.software_systems = vec![system("2", "Big Bank")];
        workspace.model.relationships = vec![rel("5", "1", "2", Some("Uses"), false)];
        workspace.views.dynamic_views.push(DynamicView {
            base: ViewBase::new("Dynamic-001", 1),
            content: ViewContent::default(),
            element_id: None,
            deployment_environment: None,
            external_boundaries_visible: false,
        });
        workspace.views.dynamic_views[0].content.relationships = vec![rv("5")];
        workspace
    }

    fn exported(workspace: &Workspace) -> Vec<Diagram> {
        export_all(workspace).expect("export")
    }

    #[test]
    fn dynamic_document_is_header_participants_blank_relationships() {
        let diagram = exported(&workspace()).pop().expect("one diagram");
        assert_eq!(diagram.file_extension(), "wsd");
        assert_eq!(diagram.key(), "Dynamic-001");
        assert_eq!(
            diagram.definition(),
            "title Dynamic View\n\
             \n\
             actor <<Person>>\\nAlice as Alice\n\
             participant <<Software System>>\\nBig Bank as Big Bank\n\
             \n\
             Alice->Big Bank: Uses\n"
        );
    }

    #[test]
    fn title_carries_the_description_after_a_literal_newline() {
        let mut ws = workspace();
        ws.views.dynamic_views[0].base.description = Some("How it flows".into());
        let diagram = exported(&ws).pop().expect("one diagram");
        assert!(
            diagram
                .definition()
                .starts_with("title Dynamic View\\nHow it flows\n\n"),
            "{}",
            diagram.definition()
        );
    }

    #[test]
    fn async_and_response_legs_pick_the_arrow_and_swap() {
        let mut ws = workspace();
        ws.model.relationships[0].interaction_style = Some(InteractionStyle::Asynchronous);
        let diagram = exported(&ws).pop().expect("one diagram");
        assert!(
            diagram.definition().contains("Alice->>Big Bank: Uses\n"),
            "{}",
            diagram.definition()
        );

        let mut ws = workspace();
        ws.model.relationships[0].interaction_style = Some(InteractionStyle::Asynchronous);
        let mut response = rv("5");
        response.response = Some(true);
        ws.views.dynamic_views[0].content.relationships = vec![response];
        let diagram = exported(&ws).pop().expect("one diagram");
        assert!(
            diagram.definition().contains("Big Bank-->>Alice: Uses\n"),
            "{}",
            diagram.definition()
        );

        // A synchronous response still reverses (`-->`).
        let mut ws = workspace();
        let mut response = rv("5");
        response.response = Some(true);
        ws.views.dynamic_views[0].content.relationships = vec![response];
        let diagram = exported(&ws).pop().expect("one diagram");
        assert!(
            diagram.definition().contains("Big Bank-->Alice: Uses\n"),
            "{}",
            diagram.definition()
        );
    }

    #[test]
    fn relationship_view_override_wins_and_null_null_is_empty() {
        let mut ws = workspace();
        let mut override_rv = rv("5");
        override_rv.description = Some("Submits credentials to".into());
        ws.views.dynamic_views[0].content.relationships = vec![override_rv];
        let diagram = exported(&ws).pop().expect("one diagram");
        assert!(
            diagram
                .definition()
                .contains("Alice->Big Bank: Submits credentials to\n"),
            "{}",
            diagram.definition()
        );

        // No override, no model description: EMPTY after the space (the official tooling
        // `A->B: ` verified case — the trailing space is part of the line).
        let mut ws = workspace();
        ws.model.relationships[0].description = None;
        let diagram = exported(&ws).pop().expect("one diagram");
        assert!(
            diagram.definition().ends_with("Alice->Big Bank: \n"),
            "{:?}",
            diagram.definition()
        );
    }

    #[test]
    fn participant_less_dynamic_view_keeps_the_unconditional_blank() {
        let mut ws = workspace();
        ws.views.dynamic_views[0].content.relationships.clear();
        let diagram = exported(&ws).pop().expect("one diagram");
        // title, header blank, the unconditional blank; no trailing blank
        // (no relationships), the render has nothing to strip beyond the
        // header blank's newline.
        assert_eq!(diagram.definition(), "title Dynamic View\n\n");
    }

    #[test]
    fn terminology_overrides_apply_but_custom_metadata_alone_rules() {
        let mut ws = workspace();
        ws.views.configuration.terminology = Terminology {
            person: Some("Actor!".into()),
            software_system: Some(" ".into()), // trim-empty → default
            ..Terminology::default()
        };
        let diagram = exported(&ws).pop().expect("one diagram");
        assert!(
            diagram
                .definition()
                .contains("actor <<Actor!>>\\nAlice as Alice"),
            "{}",
            diagram.definition()
        );
        assert!(
            diagram
                .definition()
                .contains("participant <<Software System>>\\nBig Bank as Big Bank"),
            "{}",
            diagram.definition()
        );
    }

    #[test]
    fn custom_view_exports_via_the_base_body() {
        // The verified case (`custom-view-animation`): two
        // CUSTOM elements (a `Person` here is an official type-cast abort);
        // metadata-less participants print `<<Element>>`, and
        // a description-less relationship prints `A->B: ` with the trailing
        // space.
        let mut ws = workspace();
        ws.views.dynamic_views.clear();
        ws.model.people = Vec::new();
        ws.model.software_systems = Vec::new();
        ws.model.custom_elements = vec![custom_element("1", "A"), custom_element("2", "B")];
        ws.model.relationships = vec![rel("5", "1", "2", None, false)];
        let mut custom = CustomView {
            base: ViewBase::new("Custom-001", 1),
            content: ViewContent::default(),
            animations: Vec::new(),
            software_system_id: None,
        };
        custom.content.elements = vec![
            ElementView {
                id: Id::new("1"),
                x: 0,
                y: 0,
            },
            ElementView {
                id: Id::new("2"),
                x: 0,
                y: 0,
            },
        ];
        custom.content.relationships = vec![rv("5")];
        ws.views.custom_views.push(custom);
        let diagram = exported(&ws).pop().expect("one diagram");
        assert_eq!(
            diagram.definition(),
            "title Custom View: null\n\
             \n\
             participant <<Element>>\\nA as A\n\
             participant <<Element>>\\nB as B\n\
             \n\
             A->B: \n"
        );
    }

    #[test]
    fn static_structure_views_export_nothing() {
        // The static/deployment overloads return `null`: a landscape-only
        // workspace writes no diagrams.
        let mut ws = workspace();
        ws.views.dynamic_views.clear();
        ws.views.system_landscape_views.push(SystemLandscapeView {
            base: ViewBase::new("SystemLandscape-001", 1),
            content: ViewContent::default(),
            enterprise_boundary_visible: true,
            animations: Vec::new(),
        });
        let diagrams = exported(&ws);
        assert!(diagrams.is_empty(), "{diagrams:?}");
    }
}
