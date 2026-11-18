//! The shared diagram engine — the port of `export/AbstractDiagramExporter`
//! (848 lines) + the `AbstractExporter` helpers (`breakText`, `typeOf`),
//! pinned `ad34036`. Iteration contracts: `agents/plan/build/EXPORT-PLANTUML.md` §14.
//!
//! The Java base class is abstract over the dialect's writers; the Rust
//! shape is an [`Engine`] struct (workspace + color scheme + the untyped
//! `frame` state) driving a [`Dialect`] trait of the `write*`/`start*`/
//! `end*` hooks. Element "identity" (Java never overrides `equals` — every
//! `contains`/`indexOf`/`Set` is reference-based) becomes id equality: the
//! model keeps one object per id.

use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;

use oxidizr_core::configuration::Workspace;
use oxidizr_core::model::{Model, Relationship};
use oxidizr_core::views::{
    Animation, ColorScheme, DynamicView, ElementView, MetadataSymbols, RelationshipView,
    Terminology, View, ViewBase, ViewContent,
};

use crate::diagram::{Diagram, Legend};
use crate::error::ExportError;
use crate::indent::IndentingWriter;
use crate::plantuml::java_cmp_str;
use crate::plantuml::model::{ElemRef, element_by_id};
use crate::property::export_property;

/// The `frame` field — Java's untyped `Object`: `Integer` (static/
/// deployment animation step, also used for custom views) or `String`
/// (dynamic relationship-view order). A Java `null` order stays `null`
/// (the `LinkedHashSet` keeps the null entry; `this.frame = null`
/// re-exports everything visible), so it maps to NO frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// Static-view / deployment-view animation step.
    Step(i32),
    /// Dynamic-view relationship-view order.
    Order(String),
}

/// The dialect hooks — every `protected abstract` of
/// `AbstractDiagramExporter` the engine drives, plus the two
/// `AbstractPlantUMLExporter` boundary-box hooks the sequence path uses
/// and the two gates (`isAnimationSupported`, `renderAsSequenceDiagram`).
/// The Structurizr dialect (M2-c) and C4 dialect (M3) implement this.
/// Every hook ties the engine, view and element to ONE lifetime `'a`, so
/// implementations can call the engine's element helpers
/// (`type_of`/`canonical_name`/`property`) with the element they are
/// handed.
pub trait Dialect {
    /// `writeHeader`. `Result` because the Structurizr header's title
    /// fallback (`view.getName()`) NPEs on a dangling view scope.
    ///
    /// # Errors
    ///
    /// [`ExportError::ReferenceAbort`] on the Java NPE/cast paths reached
    /// while writing the header (e.g. a dangling view scope).
    fn write_header<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError>;
    /// `writeFooter`.
    fn write_footer<'a>(&mut self, e: &Engine<'a>, view: View<'a>, w: &mut IndentingWriter);
    /// `writeElement`. `Result` because it consults `isVisible` (the Java
    /// untyped-`frame` cast) — the `hide <id>` leg.
    ///
    /// # Errors
    ///
    /// [`ExportError::ReferenceAbort`] on the Java NPE/cast paths reached
    /// while writing the element (e.g. the frame-mismatch cast).
    fn write_element<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        element: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError>;
    /// `writeRelationship`. `Result` because the model relationship behind
    /// the view must resolve (the reference would NPE).
    ///
    /// # Errors
    ///
    /// [`ExportError::ReferenceAbort`] when the relationship view
    /// references a missing model relationship.
    fn write_relationship<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        relationship_view: &'a RelationshipView,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError>;
    /// `startGroupBoundary`. `Result` because the Structurizr dialect's
    /// group-separator cut is a Java `substring` abort path.
    ///
    /// # Errors
    ///
    /// [`ExportError::ReferenceAbort`] when the group-separator cut falls
    /// outside the group name (the Java `substring` bounds check).
    fn start_group_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        group: &str,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError>;
    /// `endGroupBoundary`.
    fn end_group_boundary<'a>(&mut self, e: &Engine<'a>, view: View<'a>, w: &mut IndentingWriter);
    /// `startSoftwareSystemBoundary`.
    fn start_software_system_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        system: ElemRef<'a>,
        w: &mut IndentingWriter,
    );
    /// `endSoftwareSystemBoundary`.
    fn end_software_system_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        w: &mut IndentingWriter,
    );
    /// `startContainerBoundary`.
    fn start_container_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        container: ElemRef<'a>,
        w: &mut IndentingWriter,
    );
    /// `endContainerBoundary`.
    fn end_container_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        w: &mut IndentingWriter,
    );
    /// `startDeploymentNodeBoundary`. `Result` because the writer emits the
    /// `hide <id>` leg (the `isVisible` cast).
    ///
    /// # Errors
    ///
    /// [`ExportError::ReferenceAbort`] on the Java NPE/cast paths reached
    /// while opening the node boundary (e.g. the frame-mismatch cast).
    fn start_deployment_node_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        node: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError>;
    /// `endDeploymentNodeBoundary`.
    fn end_deployment_node_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        w: &mut IndentingWriter,
    );
    /// `AbstractPlantUMLExporter.startBoundary` (sequence-path `box`).
    fn start_boundary<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        element: ElemRef<'a>,
        w: &mut IndentingWriter,
    );
    /// `AbstractPlantUMLExporter.endBoundary`.
    fn end_boundary<'a>(&mut self, e: &Engine<'a>, view: View<'a>, w: &mut IndentingWriter);
    /// `isAnimationSupported` (base `false`; the `PlantUML` dialects read
    /// the `plantuml.animation` property). The tied `'a` lets a dialect
    /// call `e.property(view.base(), …)` (needs `&'a ViewBase`).
    fn is_animation_supported<'a>(&self, _e: &Engine<'a>, _view: View<'a>) -> bool {
        false
    }
    /// `renderAsSequenceDiagram` (base `false`; the `PlantUML` dialects
    /// gate dynamic views on `plantuml.sequenceDiagram`).
    fn render_as_sequence_diagram<'a>(&self, _e: &Engine<'a>, _view: View<'a>) -> bool {
        false
    }
    /// Whether the dialect owns its sequence syntax: the engine's
    /// `export_dynamic_sequence` is the `AbstractPlantUMLExporter` walk
    /// (`plantuml.boundaries` + the `box` stack), but a dialect that
    /// overrides `export(DynamicView)` wholesale — the Mermaid exporter,
    /// catalog `EXPORT-MERMAID.md` §9 — emits a standalone document with
    /// its own grammar and NO header/footer/legend/frames. When this is
    /// `true`, `render_as_sequence_diagram` routes to
    /// [`Dialect::sequence_document`] instead (M4).
    fn owns_sequence_document(&self) -> bool {
        false
    }
    /// The dialect-owned sequence document, consulted only when
    /// [`Dialect::owns_sequence_document`] is `true`; the default body is
    /// an abort, never reached (non-owning dialects route to the engine
    /// walk).
    ///
    /// # Errors
    ///
    /// [`ExportError::ReferenceAbort`] on the Java NPE paths of the owned
    /// walk (dangling relationship endpoints, a null-null description on
    /// the Mermaid sequence leg).
    fn sequence_document<'a>(
        &mut self,
        _e: &Engine<'a>,
        _view: View<'a>,
        _dv: &'a DynamicView,
    ) -> Result<Diagram, ExportError> {
        Err(ExportError::ReferenceAbort(
            "dialect claims its own sequence document but implements none",
        ))
    }
    /// Whether the dialect emits `CustomView` diagrams at all: the Java
    /// `C4PlantUMLExporter.export(CustomView)` overload returns `null`, and
    /// the `export(Workspace)` loop then skips the view entirely (M3;
    /// witness: the `custom-view-animation` C4 sweep writes NO files).
    fn exports_custom_views(&self) -> bool {
        true
    }
    /// `createDiagram`.
    fn create_diagram(&self, e: &Engine, view: View<'_>, definition: String) -> Diagram;
    /// `createLegend` (base returns `null`). Called AFTER the last body
    /// export, with the frame state of that export (§14 lifecycle).
    fn create_legend(&mut self, _e: &Engine, _view: View<'_>) -> Option<Legend> {
        None
    }
}

/// The shared engine state (workspace, color scheme, animation frame).
pub struct Engine<'a> {
    workspace: &'a Workspace,
    color_scheme: ColorScheme,
    frame: Option<Frame>,
    /// 2R R-exp-08: lazy id → relationship index. The reference's
    /// `getRelationshipById` scans `getRelationships()` per call; oxidizr
    /// builds the map once per export (first id wins — the `find` semantics
    /// of the old scan on a duplicate id).
    rel_index: OnceCell<HashMap<&'a str, &'a Relationship>>,
    /// 2R R-exp-08: memo for `canonical_name` (id → name; the workspace is
    /// immutable for `'a`, so the name is a pure function of the id).
    name_index: RefCell<HashMap<&'a str, String>>,
}

impl<'a> Engine<'a> {
    /// New engine over `workspace` (frame state starts empty).
    // 2R R-exp-08: no longer `const` — the memo maps need `HashMap::new()`
    // (not a `const fn`); engines are built once per export anyway.
    #[must_use]
    pub fn new(workspace: &'a Workspace, color_scheme: ColorScheme) -> Self {
        Self {
            workspace,
            color_scheme,
            frame: None,
            rel_index: OnceCell::new(),
            name_index: RefCell::new(HashMap::new()),
        }
    }

    /// The workspace being exported.
    // 2R R-exp-09 (R-core-06 shape): pub-in-crate shrink — zero callers
    // outside this crate; the getter stays as the Java parity surface
    // (`AbstractDiagramExporter#getWorkspace`).
    #[allow(dead_code)] // Java parity surface, see above
    #[must_use]
    pub(crate) const fn workspace(&self) -> &'a Workspace {
        self.workspace
    }

    /// The model (`view.getModel()`).
    #[must_use]
    pub fn model(&self) -> &'a Model {
        &self.workspace.model
    }

    /// The view configuration (styles, terminology, metadata symbols).
    #[must_use]
    pub fn configuration(&self) -> &'a oxidizr_core::views::ViewConfiguration {
        &self.workspace.views.configuration
    }

    /// The exporter color scheme.
    #[must_use]
    pub const fn color_scheme(&self) -> ColorScheme {
        self.color_scheme
    }

    /// The current animation frame (`hide` legs and `isVisible` consult
    /// it; `None` on every non-animation export).
    // 2R R-exp-09 (R-core-06 shape): zero callers outside this crate;
    // stays as the parity surface for the Java `frame` field read.
    #[allow(dead_code)] // Java parity surface, see above
    #[must_use]
    pub(crate) const fn frame(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }

    /// `getViewOrViewSetProperty` — the two-level cascade (P2-D8).
    #[must_use]
    pub fn property(&self, view: &'a ViewBase, name: &str, default: &'a str) -> &'a str {
        export_property(view, &self.workspace.views, name, default)
    }

    /// `export(Workspace)`: custom → landscape → context → container →
    /// component → dynamic → deployment views, each in stored order; image
    /// and filtered views are NOT exported (the Java loop skips them).
    ///
    /// # Errors
    ///
    /// [`ExportError::ReferenceAbort`] when a view aborts the export (the
    /// Java cast/NPE paths of §14); [`ExportError::Json`] never fires here.
    pub fn export_all<D: Dialect>(&mut self, d: &mut D) -> Result<Vec<Diagram>, ExportError> {
        let mut diagrams = Vec::new();
        for view in self.workspace.views.all_views() {
            if matches!(view, View::Filtered(_) | View::Image(_)) {
                continue;
            }
            // The Java overload returning `null` (C4 custom views, M3) makes
            // this loop skip the view — no diagram, no file.
            if matches!(view, View::Custom(_)) && !d.exports_custom_views() {
                continue;
            }
            diagrams.push(self.export_view(d, view)?);
        }
        Ok(diagrams)
    }

    /// `export(ModelView)` — dispatch plus the animation-frame and legend
    /// lifecycle of the matching per-kind `export(view)` method (the legend
    /// is created LAST, with the frame state the last body export left).
    ///
    /// # Errors
    ///
    /// [`ExportError::ReferenceAbort`] when the view aborts the export
    /// (the Java cast/NPE paths of §14).
    pub fn export_view<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
    ) -> Result<Diagram, ExportError> {
        if let View::Dynamic(dv) = view {
            if d.render_as_sequence_diagram(self, view) {
                if d.owns_sequence_document() {
                    return d.sequence_document(self, view, dv);
                }
                return self.export_dynamic_sequence(d, view, dv);
            }
            let mut diagram = self.body(d, view)?;
            if d.is_animation_supported(self, view) {
                // LinkedHashSet over the TreeSet orders: first occurrence
                // wins, insertion order; a `None` order is its own entry
                // and re-exports with frame = null (everything visible).
                let mut orders: Vec<Option<String>> = Vec::new();
                for rv in self.view_relationships(view) {
                    let order = rv.order.clone();
                    if !orders.contains(&order) {
                        orders.push(order);
                    }
                }
                for order in orders {
                    self.frame = order.map(Frame::Order);
                    diagram.add_frame(self.body(d, view)?);
                }
            }
            if let Some(legend) = d.create_legend(self, view) {
                diagram.set_legend(legend);
            }
            return Ok(diagram);
        }
        let animations: &[Animation] = match view {
            View::Custom(v) => &v.animations,
            View::Landscape(v) => &v.animations,
            View::Context(v) => &v.animations,
            View::Container(v) => &v.animations,
            View::Component(v) => &v.animations,
            View::Deployment(v) => &v.animations,
            View::Dynamic(_) | View::Filtered(_) | View::Image(_) => &[],
        };
        let mut diagram = self.body(d, view)?;
        if d.is_animation_supported(self, view) && !animations.is_empty() {
            // 2R R-exp-18: iterate directly — `animations` borrows the
            // workspace (`'a`), not the engine, so the collect-then-loop
            // bought nothing.
            for step in animations.iter().map(|a| a.order) {
                self.frame = Some(Frame::Step(step));
                diagram.add_frame(self.body(d, view)?);
            }
        }
        if let Some(legend) = d.create_legend(self, view) {
            diagram.set_legend(legend);
        }
        Ok(diagram)
    }

    /// The per-kind private `export(view, animationStep)` bodies —
    /// dispatch only (2R R-exp-11: one `body_*` fn per reference
    /// `export(view)` overload, each citing its own; the
    /// `too_many_lines` allow this dispatch shape replaces is gone).
    fn body<D: Dialect>(&mut self, d: &mut D, view: View<'a>) -> Result<Diagram, ExportError> {
        let mut w = IndentingWriter::new();
        d.write_header(self, view, &mut w)?;
        match view {
            View::Custom(_) | View::Landscape(_) | View::Context(_) => {
                self.body_flat(d, view, &mut w)?;
            }
            View::Container(_) => self.body_container(d, view, &mut w)?,
            View::Component(_) => self.body_component(d, view, &mut w)?,
            View::Dynamic(dv) => self.body_dynamic(d, view, dv, &mut w)?,
            View::Deployment(_) => self.body_deployment(d, view, &mut w)?,
            View::Filtered(_) | View::Image(_) => {
                return Err(ExportError::ReferenceAbort(
                    "filtered/image views are not exportable (Java throws)",
                ));
            }
        }
        d.write_footer(self, view, &mut w);
        Ok(d.create_diagram(self, view, w.to_string()))
    }

    /// `export(view)` for the Custom/Landscape/Context overloads: the
    /// grouped elements, a blank line iff any were written, relationships.
    fn body_flat<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let mut elements = self.get_groupable_elements(view, None)?;
        self.write_elements(d, view, &mut elements, w)?;
        if !elements.is_empty() {
            w.write_line(None);
        }
        self.write_relationships(d, view, w)
    }

    /// `export(view)` for the Container overload: the non-container
    /// (system) elements, a blank line iff any, then one
    /// software-system boundary per boundary-system.
    fn body_container<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let mut elements_written = false;
        for element in self.view_elements(view) {
            if element.kind() != oxidizr_core::validate::ElementKind::Container {
                d.write_element(self, view, element, w)?;
                elements_written = true;
            }
        }
        if elements_written {
            w.write_line(None);
        }
        for system in self.get_boundary_systems(view) {
            d.start_software_system_boundary(self, view, system, w);
            let mut scoped = self.get_groupable_elements(view, Some(system.id()))?;
            self.write_elements(d, view, &mut scoped, w)?;
            d.end_software_system_boundary(self, view, w);
        }
        self.write_relationships(d, view, w)
    }

    /// `export(view)` for the Component overload: the flat custom/people/
    /// systems legs, the boundary + container legs, relationships.
    fn body_component<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let customs = self.get_custom_elements(view);
        self.write_flat_leg(d, view, &customs, w)?;
        let people = self.get_people(view);
        self.write_flat_leg(d, view, &people, w)?;
        let systems = self.get_systems(view);
        self.write_flat_leg(d, view, &systems, w)?;
        self.write_boundary_container_legs(d, view, w)?;
        self.write_relationships(d, view, w)
    }

    /// `export(view)` for the Dynamic overload: the elements scoped by the
    /// dynamic `elementId` (none → grouped; `SoftwareSystem` → boundaries
    /// then rootless; `Container` → the component legs then
    /// non-components), a blank line iff any were written, relationships.
    fn body_dynamic<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        dv: &'a DynamicView,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let mut elements_written = false;
        let scope = dv
            .element_id
            .as_ref()
            .and_then(|id| element_by_id(self.model(), id));
        match scope.map(ElemRef::kind) {
            None => {
                let mut elements = self.get_groupable_elements(view, None)?;
                self.write_elements(d, view, &mut elements, w)?;
                elements_written = !elements.is_empty();
            }
            Some(oxidizr_core::validate::ElementKind::SoftwareSystem) => {
                for system in self.get_boundary_systems(view) {
                    d.start_software_system_boundary(self, view, system, w);
                    let mut scoped = self.get_groupable_elements(view, Some(system.id()))?;
                    self.write_elements(d, view, &mut scoped, w)?;
                    d.end_software_system_boundary(self, view, w);
                }
                for element in self.view_elements(view) {
                    if element.parent_id().is_none() {
                        d.write_element(self, view, element, w)?;
                        elements_written = true;
                    }
                }
            }
            Some(oxidizr_core::validate::ElementKind::Container) => {
                let customs = self.get_custom_elements(view);
                self.write_flat_leg(d, view, &customs, w)?;
                let people = self.get_people(view);
                self.write_flat_leg(d, view, &people, w)?;
                let systems = self.get_systems(view);
                self.write_flat_leg(d, view, &systems, w)?;
                self.write_boundary_container_legs(d, view, w)?;
                for element in self.view_elements(view) {
                    // Java: `!(parent instanceof Container)` — the
                    // kinds with a Container parent are components.
                    if element.kind() != oxidizr_core::validate::ElementKind::Component {
                        d.write_element(self, view, element, w)?;
                        elements_written = true;
                    }
                }
            }
            Some(_) => {}
        }
        if elements_written {
            w.write_line(None);
        }
        self.write_relationships(d, view, w)
    }

    /// `export(view)` for the Deployment overload: grouped elements then
    /// relationships — NO blank-line leg between them.
    fn body_deployment<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let mut elements = self.get_groupable_elements(view, None)?;
        self.write_elements(d, view, &mut elements, w)?;
        self.write_relationships(d, view, w)
    }

    /// A custom/people/systems leg: every element, then a blank line iff
    /// the list was non-empty (the `export(ComponentView)` pattern).
    fn write_flat_leg<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        elements: &[ElemRef<'a>],
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        for element in elements {
            d.write_element(self, view, *element, w)?;
        }
        if !elements.is_empty() {
            w.write_line(None);
        }
        Ok(())
    }

    /// The component-view boundary sequence (also the container-scoped
    /// dynamic leg): boundary containers id-sorted, their owners deduped
    /// in first-seen order; per owner the container boundaries with their
    /// scoped components, then the owner's view containers written flat.
    fn write_boundary_container_legs<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let boundary_containers = self.get_boundary_containers(view);
        let mut owners: Vec<&oxidizr_core::Id> = Vec::new();
        for container in &boundary_containers {
            let owner = container.owner_system_id();
            if !owners.contains(&owner) {
                owners.push(owner);
            }
        }
        let containers = self.get_containers(view);
        for owner in owners {
            let owner_ref = element_by_id(self.model(), owner)
                .ok_or(ExportError::ReferenceAbort("dangling container owner id"))?;
            d.start_software_system_boundary(self, view, owner_ref, w);
            for container in &boundary_containers {
                if container.owner_system_id() == owner {
                    d.start_container_boundary(self, view, *container, w);
                    let mut scoped = self.get_groupable_elements(view, Some(container.id()))?;
                    self.write_elements(d, view, &mut scoped, w)?;
                    d.end_container_boundary(self, view, w);
                }
            }
            for container in &containers {
                if container.owner_system_id() == owner {
                    d.write_element(self, view, *container, w)?;
                }
            }
            d.end_software_system_boundary(self, view, w);
        }
        Ok(())
    }

    /// `AbstractPlantUMLExporter.export(DynamicView)` sequence-diagram body
    /// (catalog §8): header, element set + boundary stack (or flat), blank
    /// line iff elements, relationships, footer, then the legend LAST.
    // Mirrors the single Java method; the boundary-stack walk stays inline.
    /// 2R R-exp-11: the two inner blocks of this walk are the
    /// `sequence_write_order` / `write_sequence_boundaries` siblings;
    /// `export_dynamic_sequence` itself is cited by catalog name, so the
    /// fn and its name stay.
    fn export_dynamic_sequence<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        dv: &'a DynamicView,
    ) -> Result<Diagram, ExportError> {
        let mut w = IndentingWriter::new();
        d.write_header(self, view, &mut w)?;

        // LinkedHashSet: source-then-destination per TreeSet relationship.
        let mut elements: Vec<ElemRef<'a>> = Vec::new();
        for rv in self.view_relationships(view) {
            let relationship = self.relationship(rv)?;
            for id in [&relationship.source, &relationship.destination] {
                let element = element_by_id(self.model(), id).ok_or(
                    ExportError::ReferenceAbort("dangling relationship endpoint"),
                )?;
                if !elements.iter().any(|e| e.id() == element.id()) {
                    elements.push(element);
                }
            }
        }

        let boundaries = is_true_ci(self.property(
            &dv.base,
            crate::plantuml::PLANTUML_BOUNDARIES_PROPERTY,
            "true",
        ));
        if boundaries {
            // 2R R-exp-11: the ordering and the box walk of this leg are
            // the `sequence_write_order` + `write_sequence_boundaries`
            // siblings (the Java `elementsInWriteOrder` map walk, then
            // the boundary open/close stack).
            let order = self.sequence_write_order(&elements)?;
            self.write_sequence_boundaries(d, view, &elements, &order, &mut w)?;
        } else {
            for element in &elements {
                d.write_element(self, view, *element, &mut w)?;
            }
        }

        if !elements.is_empty() {
            w.write_line(None);
        }
        self.write_relationships(d, view, &mut w)?;
        d.write_footer(self, view, &mut w);
        let mut diagram = d.create_diagram(self, view, w.to_string());
        if let Some(legend) = d.create_legend(self, view) {
            diagram.set_legend(legend);
        }
        Ok(diagram)
    }

    /// The Java `elementsInWriteOrder` map walk of the dynamic-sequence
    /// boundaries leg (2R R-exp-11 split of `export_dynamic_sequence`):
    /// rootless elements first-seen; else the parent chain (root-last
    /// reversed) appended, then the element inserted after its last
    /// canonical-name "sibling" (the `contains(parentCanonical.
    /// substring(indexOf("://")))` test) or after its parent when there
    /// are none.
    fn sequence_write_order(
        &self,
        elements: &[ElemRef<'a>],
    ) -> Result<Vec<ElemRef<'a>>, ExportError> {
        let mut order: Vec<ElemRef<'a>> = Vec::new();
        for element in elements {
            let Some(parent_id) = element.parent_id().cloned() else {
                order.push(*element);
                continue;
            };
            let mut parents: Vec<ElemRef<'a>> = Vec::new();
            let mut current = element_by_id(self.model(), &parent_id)
                .ok_or(ExportError::ReferenceAbort("dangling parent id"))?;
            loop {
                parents.push(current);
                let Some(next) = current.parent_id().cloned() else {
                    break;
                };
                current = element_by_id(self.model(), &next)
                    .ok_or(ExportError::ReferenceAbort("dangling parent id"))?;
            }
            for p in parents.iter().rev() {
                if !order.iter().any(|e| e.id() == p.id()) {
                    order.push(*p);
                }
            }
            let needle = parents.first().map_or(String::new(), |p| {
                self.canonical_name(*p)
                    .split_once("://")
                    .map_or(String::new(), |(_, rest)| format!("://{rest}"))
            });
            let siblings: Vec<usize> = order
                .iter()
                .enumerate()
                .filter(|(_, e)| self.canonical_name(**e).contains(&needle))
                .map(|(i, _)| i)
                .collect();
            let index = if let Some(last) = siblings.last() {
                *last + 1
            } else {
                order
                    .iter()
                    .position(|e| e.id() == &parent_id)
                    .ok_or(ExportError::ReferenceAbort("dangling parent id"))?
                    + 1
            };
            order.insert(index, *element);
        }
        Ok(order)
    }

    /// The boundary open/close stack of the dynamic-sequence boundaries leg
    /// (2R R-exp-11 split of `export_dynamic_sequence`): walk
    /// `sequence_write_order` — a view element inside the current box (or
    /// with no box open) writes directly, anything else unwinds the stack
    /// to the matching parent first; non-view elements open boundary boxes
    /// (`startBoundary`/`endBoundary`), drained at the end.
    fn write_sequence_boundaries<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        elements: &[ElemRef<'a>],
        order: &[ElemRef<'a>],
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let mut stack: Vec<ElemRef<'a>> = Vec::new();
        for element in order {
            let at_parent = |stack: &Vec<ElemRef<'a>>| {
                stack
                    .last()
                    .is_some_and(|top| element.parent_id() == Some(top.id()))
            };
            if elements.iter().any(|e| e.id() == element.id()) {
                if stack.is_empty() || at_parent(&stack) {
                    d.write_element(self, view, *element, w)?;
                } else {
                    while !stack.is_empty() && !at_parent(&stack) {
                        stack.pop();
                        d.end_boundary(self, view, w);
                    }
                    d.write_element(self, view, *element, w)?;
                }
            } else {
                while !stack.is_empty() && !at_parent(&stack) {
                    stack.pop();
                    d.end_boundary(self, view, w);
                }
                d.start_boundary(self, view, *element, w);
                stack.push(*element);
            }
        }
        while stack.pop().is_some() {
            d.end_boundary(self, view, w);
        }
        Ok(())
    }

    /// `writeElements` — the grouped pass (`findGroups` has already sorted
    /// `elements` IN PLACE by id string; the ungrouped pass sees that
    /// order), then the ungrouped elements.
    fn write_elements<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        elements: &mut Vec<ElemRef<'a>>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let groups = self.find_groups(elements);
        if !groups.is_empty() {
            let separator = self.group_separator();
            if is_value(&separator) {
                // Nested mode — the upstream context-tracking quirks are
                // mirrored EXACTLY (catalog §14).
                let mut context = String::new();
                for group in &groups {
                    let group_count = java_split_len(group, &separator);
                    let context_count = java_split_len(&context, &separator);
                    match group_count.cmp(&context_count) {
                        std::cmp::Ordering::Greater => w.indent(),
                        std::cmp::Ordering::Equal => {
                            // Upstream quirk: a ONE-character context never
                            // closes the previous group (UTF-16 length > 1).
                            if context.encode_utf16().count() > 1 {
                                d.end_group_boundary(self, view, w);
                            }
                        }
                        std::cmp::Ordering::Less => {
                            for _ in 0..(context_count - group_count) {
                                d.end_group_boundary(self, view, w);
                                w.outdent();
                            }
                            d.end_group_boundary(self, view, w);
                        }
                    }
                    d.start_group_boundary(self, view, group, w)?;
                    for element in elements.iter().copied() {
                        if element.group() == Some(group.as_str()) {
                            self.write(d, view, element, w)?;
                        }
                    }
                    context.clone_from(group);
                }
                let context_count = java_split_len(&context, &separator);
                for i in 0..context_count {
                    d.end_group_boundary(self, view, w);
                    if i < context_count - 1 {
                        w.outdent();
                    }
                }
            } else {
                for group in &groups {
                    d.start_group_boundary(self, view, group, w)?;
                    for element in elements.iter().copied() {
                        if element.group() == Some(group.as_str()) {
                            self.write(d, view, element, w)?;
                        }
                    }
                    d.end_group_boundary(self, view, w);
                }
            }
        }

        for element in elements.iter().copied() {
            if is_null_or_empty(element.group()) {
                self.write(d, view, element, w)?;
            }
        }
        Ok(())
    }

    /// The private `write`: deployment nodes recurse their boundary.
    fn write<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        element: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        if matches!(view, View::Deployment(_))
            && element.kind() == oxidizr_core::validate::ElementKind::DeploymentNode
        {
            self.write_deployment_node(d, view, element, w)?;
        } else {
            d.write_element(self, view, element, w)?;
        }
        Ok(())
    }

    /// The `writeDeploymentNode` recursion: children/infrastructure/
    /// instances each NAME-sorted (stable), filtered to view members,
    /// concatenated — `writeElements` then re-sorts the combined list by
    /// id string (and honors groups per node).
    fn write_deployment_node<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        node: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        d.start_deployment_node_boundary(self, view, node, w)?;
        let mut elements: Vec<ElemRef<'a>> = Vec::new();
        if let ElemRef::DeploymentNode { elem, .. } = node {
            let mut children: Vec<ElemRef<'a>> = elem
                .children
                .iter()
                .map(|c| ElemRef::DeploymentNode {
                    elem: c,
                    parent: Some(elem),
                })
                .collect();
            children.sort_by(|a, b| java_cmp_str(a.name(), b.name()));
            elements.extend(
                children
                    .into_iter()
                    .filter(|e| self.is_element_in_view(view, e.id())),
            );
            let mut infra: Vec<ElemRef<'a>> = elem
                .infrastructure_nodes
                .iter()
                .map(|i| ElemRef::InfrastructureNode {
                    elem: i,
                    node: elem,
                })
                .collect();
            infra.sort_by(|a, b| java_cmp_str(a.name(), b.name()));
            elements.extend(
                infra
                    .into_iter()
                    .filter(|e| self.is_element_in_view(view, e.id())),
            );
            let mut system_instances: Vec<ElemRef<'a>> = elem
                .software_system_instances
                .iter()
                .map(|si| ElemRef::SystemInstance {
                    elem: si,
                    node: elem,
                })
                .collect();
            system_instances.sort_by(|a, b| java_cmp_str(a.name(), b.name()));
            elements.extend(
                system_instances
                    .into_iter()
                    .filter(|e| self.is_element_in_view(view, e.id())),
            );
            let mut container_instances: Vec<ElemRef<'a>> = elem
                .container_instances
                .iter()
                .map(|ci| ElemRef::ContainerInstance {
                    elem: ci,
                    node: elem,
                })
                .collect();
            container_instances.sort_by(|a, b| java_cmp_str(a.name(), b.name()));
            elements.extend(
                container_instances
                    .into_iter()
                    .filter(|e| self.is_element_in_view(view, e.id())),
            );
        }
        self.write_elements(d, view, &mut elements, w)?;
        d.end_deployment_node_boundary(self, view, w);
        Ok(())
    }

    /// `findGroups`: sorts `elements` in place by id STRING; collects the
    /// distinct groups (+ every nested ancestor prefix when a separator is
    /// set); returns them natural-string (UTF-16) sorted.
    fn find_groups(&self, elements: &mut [ElemRef<'a>]) -> Vec<String> {
        elements.sort_by(|a, b| java_cmp_str(a.id().as_str(), b.id().as_str()));
        let separator = self.group_separator();
        let nested = is_value(&separator);
        let mut groups: Vec<String> = Vec::new();
        let push = |groups: &mut Vec<String>, group: &str| {
            if !groups.iter().any(|g| g == group) {
                groups.push(group.to_string());
            }
        };
        for element in elements.iter() {
            let group = element.group().unwrap_or_default();
            if is_value(group) {
                push(&mut groups, group);
                if nested {
                    let mut prefix = group.to_string();
                    while prefix.contains(separator.as_str()) {
                        let cut = prefix.rfind(separator.as_str()).unwrap_or(0);
                        prefix.truncate(cut);
                        push(&mut groups, &prefix);
                    }
                }
            }
        }
        groups.sort_by(|a, b| java_cmp_str(a, b));
        groups
    }

    /// `Model.GROUP_SEPARATOR_PROPERTY_NAME` — the raw model property
    /// (NOT the view cascade).
    #[must_use]
    pub(crate) fn group_separator(&self) -> String {
        self.workspace
            .model
            .properties
            .get("structurizr.groupSeparator")
            .map(str::to_owned)
            .unwrap_or_default()
    }

    /// `getRelationshipsInView`: dynamic → `TreeSet` order; else the same
    /// set sorted by relationship-id string.
    fn get_relationships_in_view(&self, view: View<'a>) -> Vec<&'a RelationshipView> {
        let mut rels = self.view_relationships(view);
        if !matches!(view, View::Dynamic(_)) {
            rels.sort_by(|a, b| java_cmp_str(a.id.as_str(), b.id.as_str()));
        }
        rels
    }

    /// `writeRelationships` — ALL relationship views (never filtered by
    /// `isVisible`), + a trailing blank line iff non-empty.
    fn write_relationships<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let rels = self.get_relationships_in_view(view);
        // 2R R-exp-02: the emptiness test is hoisted before the loop, so the
        // vec moves into the iterator — no `clone()` for a trailing
        // `is_empty()` (the Java `isEmpty()` runs on the same list).
        let non_empty = !rels.is_empty();
        for rv in rels {
            d.write_relationship(self, view, rv, w)?;
        }
        if non_empty {
            w.write_line(None);
        }
        Ok(())
    }

    /// `getGroupableElements(view, parent)` — the per-kind matrix
    /// (`EXPORT-PLANTUML.md` §14). The Java cast-aborts (`ClassCastException`)
    /// are surfaced as [`ExportError::ReferenceAbort`].
    fn get_groupable_elements(
        &self,
        view: View<'a>,
        parent: Option<&oxidizr_core::Id>,
    ) -> Result<Vec<ElemRef<'a>>, ExportError> {
        use oxidizr_core::validate::ElementKind as K;
        let mut elements: Vec<ElemRef<'a>> = Vec::new();
        for ev in Self::view_element_views(view) {
            let element = element_by_id(self.model(), &ev.id)
                .ok_or(ExportError::ReferenceAbort("dangling view element id"))?;
            let k = element.kind();
            match view {
                View::Custom(_) => {
                    if k != K::CustomElement {
                        return Err(ExportError::ReferenceAbort(
                            "custom view contains a non-custom element (Java ClassCastException)",
                        ));
                    }
                    elements.push(element);
                }
                View::Landscape(_) | View::Context(_) => elements.push(element),
                View::Container(_) => {
                    if k == K::Container {
                        elements.push(element);
                    }
                }
                View::Component(_) => {
                    if k == K::Component {
                        elements.push(element);
                    }
                }
                View::Dynamic(dv) => {
                    let scope = dv
                        .element_id
                        .as_ref()
                        .and_then(|id| element_by_id(self.model(), id));
                    match scope.map(ElemRef::kind) {
                        None => {
                            if !(k == K::Person
                                || k == K::SoftwareSystem
                                || k == K::Container
                                || k == K::Component)
                            {
                                return Err(ExportError::ReferenceAbort(
                                    "dynamic view (no scope) contains a non-static-structure element (Java ClassCastException)",
                                ));
                            }
                            elements.push(element);
                        }
                        Some(K::SoftwareSystem) => {
                            if k == K::Container {
                                elements.push(element);
                            }
                        }
                        Some(K::Container) => {
                            if k == K::Component {
                                elements.push(element);
                            }
                        }
                        Some(_) => {}
                    }
                }
                View::Deployment(_) => {
                    if k == K::DeploymentNode && element.parent_id().is_none() {
                        elements.push(element);
                    }
                }
                View::Filtered(_) | View::Image(_) => {}
            }
        }
        if let Some(parent) = parent {
            elements.retain(|e| e.parent_id() == Some(parent));
        }
        Ok(elements)
    }

    /// `view.getElements()` — the `TreeSet<ElementView>` order, deduped by
    /// comparator key (first insertion wins, as in a `TreeSet`).
    #[must_use]
    pub fn view_elements(&self, view: View<'a>) -> Vec<ElemRef<'a>> {
        Self::view_element_views(view)
            .iter()
            .filter_map(|ev| element_by_id(self.model(), &ev.id))
            .collect()
    }

    /// The raw sorted element views (dangling ids kept — resolution
    /// happens per call site so a dangling id can abort where Java would).
    fn view_element_views(view: View<'a>) -> Vec<&'a ElementView> {
        let Some(content) = content_of(view) else {
            return Vec::new();
        };
        let mut evs: Vec<&'a ElementView> = content.elements.iter().collect();
        evs.sort_by(|a, b| element_view_cmp(a, b));
        dedup_by_key(&mut evs, |e| e.id.as_str());
        evs
    }

    /// `view.getRelationships()` — the `TreeSet<RelationshipView>` order
    /// (`id + "/" + order`, a null order joining as the literal `null`),
    /// deduped by key (first insertion wins). Dynamic views apply the
    /// `DynamicView.getRelationships()` override: a stable re-sort by numeric
    /// `order` when EVERY order parses as a double, else by `order` text —
    /// stability preserves the `10/3, 11/3` tie (`parallel1` witness
    /// 2026-11-17) because Java's `List.sort` (`TimSort`) and Rust's
    /// `sort_by` are both stable.
    #[must_use]
    pub fn view_relationships(&self, view: View<'a>) -> Vec<&'a RelationshipView> {
        let Some(content) = content_of(view) else {
            return Vec::new();
        };
        let mut rvs: Vec<&'a RelationshipView> = content.relationships.iter().collect();
        rvs.sort_by(|a, b| java_cmp_str(&relationship_view_key(a), &relationship_view_key(b)));
        dedup_by_key(&mut rvs, |r| relationship_view_key(r));
        if matches!(view, View::Dynamic(_)) {
            // `Double.parseDouble(null)` throws, so a missing order forces
            // the text branch — where the reference NPEs comparing nulls;
            // oxidizr sorts a missing order first as the empty string
            // (never-crash policy, same as the JSON writer).
            let numeric = rvs.iter().all(|rv| {
                rv.order
                    .as_deref()
                    .is_some_and(|o| o.parse::<f64>().is_ok())
            });
            if numeric {
                rvs.sort_by(|a, b| {
                    let x = a.order.as_deref().unwrap_or_default().parse::<f64>();
                    let y = b.order.as_deref().unwrap_or_default().parse::<f64>();
                    x.unwrap_or(0.0).total_cmp(&y.unwrap_or(0.0))
                });
            } else {
                rvs.sort_by(|a, b| {
                    java_cmp_str(
                        a.order.as_deref().unwrap_or_default(),
                        b.order.as_deref().unwrap_or_default(),
                    )
                });
            }
        }
        rvs
    }

    /// `getBoundarySoftwareSystems` — the distinct owners of the view's
    /// containers, id-string sorted.
    pub(crate) fn get_boundary_systems(&self, view: View<'a>) -> Vec<ElemRef<'a>> {
        let mut systems: Vec<&oxidizr_core::Id> = Vec::new();
        for element in self.view_elements(view) {
            if let Some(system) = element.system() {
                let id = &system.item.id;
                if !systems.contains(&id) {
                    systems.push(id);
                }
            }
        }
        systems.sort_by(|a, b| java_cmp_str(a.as_str(), b.as_str()));
        systems
            .into_iter()
            .filter_map(|id| element_by_id(self.model(), id))
            .collect()
    }

    /// `getCustomElements` (distinct first-seen, id-string sorted).
    fn get_custom_elements(&self, view: View<'a>) -> Vec<ElemRef<'a>> {
        self.filter_kind_sorted(view, oxidizr_core::validate::ElementKind::CustomElement)
    }

    /// `getPeople`.
    fn get_people(&self, view: View<'a>) -> Vec<ElemRef<'a>> {
        self.filter_kind_sorted(view, oxidizr_core::validate::ElementKind::Person)
    }

    /// `getSoftwareSystems`.
    fn get_systems(&self, view: View<'a>) -> Vec<ElemRef<'a>> {
        self.filter_kind_sorted(view, oxidizr_core::validate::ElementKind::SoftwareSystem)
    }

    /// `getContainers` — the view's containers, id-string sorted.
    fn get_containers(&self, view: View<'a>) -> Vec<ElemRef<'a>> {
        self.filter_kind_sorted(view, oxidizr_core::validate::ElementKind::Container)
    }

    /// `getBoundaryContainers` — the distinct containers owning the view's
    /// components, id-string sorted.
    pub(crate) fn get_boundary_containers(&self, view: View<'a>) -> Vec<ElemRef<'a>> {
        let mut out: Vec<ElemRef<'a>> = Vec::new();
        for element in self.view_elements(view) {
            if element.kind() == oxidizr_core::validate::ElementKind::Component {
                if let Some(parent_id) = element.parent_id() {
                    if let Some(container) = element_by_id(self.model(), parent_id) {
                        if !out.iter().any(|c| c.id() == container.id()) {
                            out.push(container);
                        }
                    }
                }
            }
        }
        out.sort_by(|a, b| java_cmp_str(a.id().as_str(), b.id().as_str()));
        out
    }

    fn filter_kind_sorted(
        &self,
        view: View<'a>,
        kind: oxidizr_core::validate::ElementKind,
    ) -> Vec<ElemRef<'a>> {
        let mut out: Vec<ElemRef<'a>> = Vec::new();
        for element in self.view_elements(view) {
            if element.kind() == kind && !out.iter().any(|e| e.id() == element.id()) {
                out.push(element);
            }
        }
        out.sort_by(|a, b| java_cmp_str(a.id().as_str(), b.id().as_str()));
        out
    }

    /// `view.isElementInView(element)` — membership by id.
    #[must_use]
    pub fn is_element_in_view(&self, view: View<'_>, id: &oxidizr_core::Id) -> bool {
        view.content()
            .is_some_and(|c| c.elements.iter().any(|e| &e.id == id))
    }

    /// The model relationship behind a relationship view (abort on a
    /// dangling id, where the reference would NPE).
    ///
    /// # Errors
    ///
    /// [`ExportError::ReferenceAbort`] when the id references a missing
    /// relationship.
    pub fn relationship(&self, rv: &RelationshipView) -> Result<&'a Relationship, ExportError> {
        // 2R R-exp-08: lazy `HashMap` — the old linear scan ran per
        // relationship view AND per `is_visible` leg (the reference reads
        // its object graph instead of re-scanning). First id wins, matching
        // the `find` semantics on a duplicate id.
        let index = self.rel_index.get_or_init(|| {
            let mut index = HashMap::with_capacity(self.model().relationships.len());
            for rel in &self.model().relationships {
                index.entry(rel.id.as_str()).or_insert(rel);
            }
            index
        });
        index
            .get(rv.id.as_str())
            .copied()
            .ok_or(ExportError::ReferenceAbort(
                "relationship view references a missing relationship",
            ))
    }

    /// `isVisible(view, element)` — animation-frame visibility ONLY (the
    /// JSON `ElementView.visible` flag is never consulted; catalog §4).
    ///
    /// # Errors
    ///
    /// [`ExportError::ReferenceAbort`] on a frame/view kind mismatch (the
    /// Java untyped-`frame` cast failing).
    pub fn is_visible(&self, view: View<'_>, element: ElemRef<'_>) -> Result<bool, ExportError> {
        // 2R R-exp-01: borrow the frame — the old shape cloned the whole
        // Frame per element×frame call (the Java field read is a reference).
        let Some(frame) = self.frame.as_ref() else {
            return Ok(true);
        };
        match frame {
            Frame::Step(step) => {
                let step = *step;
                match view {
                    View::Landscape(_)
                    | View::Context(_)
                    | View::Container(_)
                    | View::Component(_) => {
                        if step > 0 {
                            Ok(Self::revealed_by_step(view, step, element.id().as_str()))
                        } else {
                            Ok(true)
                        }
                    }
                    View::Deployment(v) => {
                        if step > 0 {
                            Ok(v.animations
                                .iter()
                                .filter(|a| a.order <= step)
                                .flat_map(|a| a.element_ids.iter())
                                .any(|id| id == element.id().as_str()))
                        } else {
                            Ok(true)
                        }
                    }
                    View::Custom(_) => Ok(true),
                    View::Dynamic(_) | View::Filtered(_) | View::Image(_) => {
                        Err(ExportError::ReferenceAbort(
                            "integer frame on a non-static view (Java ClassCastException)",
                        ))
                    }
                }
            }
            Frame::Order(order) => match view {
                View::Dynamic(dv) => {
                    let mut ids: Vec<&str> = Vec::new();
                    for rv in &dv.content.relationships {
                        if rv.order.as_deref() == Some(order.as_str()) {
                            let rel = self.relationship(rv)?;
                            ids.push(rel.source.as_str());
                            ids.push(rel.destination.as_str());
                        }
                    }
                    Ok(ids.contains(&element.id().as_str()))
                }
                _ => Err(ExportError::ReferenceAbort(
                    "string frame on a non-dynamic view (Java ClassCastException)",
                )),
            },
        }
    }

    fn revealed_by_step(view: View<'_>, step: i32, id: &str) -> bool {
        let animations = match view {
            View::Landscape(v) => &v.animations,
            View::Context(v) => &v.animations,
            View::Container(v) => &v.animations,
            View::Component(v) => &v.animations,
            _ => return false,
        };
        animations
            .iter()
            .filter(|a| a.order <= step)
            .flat_map(|a| a.element_ids.iter())
            .any(|e| e == id)
    }

    /// `AbstractExporter.typeOf` — the terminology + metadata-symbols type
    /// string: custom elements report their raw `metadata` (terminology
    /// never applies); instances get no branch and render empty; the
    /// `: <technology>` suffix is container/component/DN/IN only
    /// (catalog §14).
    #[must_use]
    pub fn type_of(&self, element: ElemRef<'a>, include_metadata_symbols: bool) -> String {
        use oxidizr_core::validate::ElementKind as K;
        let config = self.configuration();
        let terminology = |element: ElemRef<'_>| -> String {
            let t: &Terminology = &config.terminology;
            let (value, fallback): (&Option<String>, &str) = match element.kind() {
                K::Person => (&t.person, "Person"),
                K::SoftwareSystem => (&t.software_system, "Software System"),
                K::Container => (&t.container, "Container"),
                K::Component => (&t.component, "Component"),
                K::DeploymentNode => (&t.deployment_node, "Deployment Node"),
                K::InfrastructureNode => (&t.infrastructure_node, "Infrastructure Node"),
                _ => return String::new(),
            };
            value
                .as_deref()
                .filter(|s| is_value(s))
                .unwrap_or(fallback)
                .to_string()
        };
        let type_ = match element.kind() {
            K::Person | K::SoftwareSystem => terminology(element),
            K::Container | K::Component | K::DeploymentNode | K::InfrastructureNode => {
                let tech = element.technology().filter(|t| is_value(t));
                match tech {
                    Some(t) => format!("{}: {t}", terminology(element)),
                    None => terminology(element),
                }
            }
            K::CustomElement => element.metadata().unwrap_or("").to_string(),
            K::SoftwareSystemInstance | K::ContainerInstance => String::new(),
        };
        if is_null_or_empty(Some(&type_)) {
            return type_;
        }
        if include_metadata_symbols {
            // A NULL metadataSymbols setting is MUTATED to SquareBrackets
            // in the reference; here the default read is equivalent.
            match config
                .metadata_symbols
                .unwrap_or(MetadataSymbols::SquareBrackets)
            {
                MetadataSymbols::RoundBrackets => format!("({type_})"),
                MetadataSymbols::CurlyBrackets => format!("{{{type_}}}"),
                MetadataSymbols::AngleBrackets => format!("<{type_}>"),
                MetadataSymbols::DoubleAngleBrackets => format!("<<{type_}>>"),
                MetadataSymbols::None => type_,
                MetadataSymbols::SquareBrackets => format!("[{type_}]"),
            }
        } else {
            type_
        }
    }

    /// `AbstractExporter.breakText`: `maxCharacters = (int)(maxWidth /
    /// (fontSize x 0.6))`, unchanged when `length < maxCharacters` (UTF-16),
    /// word-wrap with `<br />` joins; null/trim-empty → `""`.
    // The f64→i64 conversion below mirrors Java's `(int)` truncation.
    #[allow(clippy::cast_possible_truncation)]
    #[must_use]
    pub fn break_text(&self, max_width: i32, font_size: i32, text: &str) -> String {
        if is_null_or_empty(Some(text)) {
            return String::new();
        }
        let character_width = f64::from(font_size) * 0.6;
        let max_characters = (f64::from(max_width) / character_width) as i64; // Java (int) truncates
        if utf16_len(text) < max_characters {
            return text.to_string();
        }
        // Java `split(" ")`: single-space regex, trailing empties dropped.
        let mut words: Vec<&str> = text.split(' ').collect();
        while words.last() == Some(&"") {
            words.pop();
        }
        let mut buf = String::new();
        let mut line: Option<String> = None;
        for word in words {
            match line.take() {
                None => line = Some(word.to_string()),
                Some(current) => {
                    if utf16_len(&current) + utf16_len(word) + 1 < max_characters {
                        line = Some(format!("{current} {word}"));
                    } else {
                        buf.push_str(&current);
                        buf.push_str("<br />");
                        line = Some(word.to_string());
                    }
                }
            }
        }
        if let Some(line) = line {
            buf.push_str(&line);
        }
        buf
    }

    /// `Element.getCanonicalName` (`model/CanonicalNameGenerator`):
    /// `<Kind>://<name path>` with `.` and `/` stripped from every name;
    /// deployment chains are `/`-joined, static chains `.`-joined
    /// (catalog §8 sibling matching).
    ///
    /// 2R R-exp-08: memoized per engine (id → name). The workspace is
    /// immutable for `'a`, so the name is a pure function of the id; the
    /// sequence legs and the §8 sibling matching re-probe the same elements
    /// and the deployment leg recursed per ancestor without sharing work.
    #[must_use]
    pub fn canonical_name(&self, element: ElemRef<'a>) -> String {
        let id = element.id().as_str();
        if let Some(name) = self.name_index.borrow().get(id) {
            return name.clone();
        }
        let name = self.canonical_name_uncached(element);
        // The borrow ends before the insert's own statement — `node_tail`
        // recursion re-enters this wrapper, never holds the borrow.
        self.name_index.borrow_mut().insert(id, name.clone());
        name
    }

    fn canonical_name_uncached(&self, element: ElemRef<'a>) -> String {
        use oxidizr_core::validate::ElementKind as K;
        let format_name = |name: &str| name.replace(['.', '/'], "");
        let by_id =
            |id: Option<&'a oxidizr_core::Id>| id.and_then(|id| element_by_id(self.model(), id));
        let node_tail = |node: ElemRef<'a>| -> String {
            self.canonical_name(node)
                .strip_prefix("DeploymentNode://")
                .unwrap_or_default()
                .to_string()
        };
        match element.kind() {
            K::CustomElement => format!("Custom://{}", format_name(element.name())),
            K::Person => format!("Person://{}", format_name(element.name())),
            K::SoftwareSystem => format!("SoftwareSystem://{}", format_name(element.name())),
            K::Container => {
                let system = element.system().map_or("", |s| s.item.name.as_str());
                format!(
                    "Container://{}.{}",
                    format_name(system),
                    format_name(element.name())
                )
            }
            K::Component => {
                let container = by_id(element.parent_id());
                let (system_name, container_name) =
                    container.map_or((String::new(), element.name().to_string()), |c| {
                        let s = c.system().map_or("", |s| s.item.name.as_str());
                        (s.to_string(), c.name().to_string())
                    });
                format!(
                    "Component://{}.{}.{}",
                    format_name(&system_name),
                    format_name(&container_name),
                    format_name(element.name())
                )
            }
            K::DeploymentNode => {
                let mut chain: Vec<String> = Vec::new();
                let mut current = element;
                while let Some(parent) = by_id(current.parent_id()) {
                    chain.push(format_name(parent.name()));
                    current = parent;
                }
                chain.reverse();
                let env = element.environment().unwrap_or("");
                // 2R R-exp-13: one `join` instead of the per-name push loop.
                // The trailing slash belongs to the chain, so an EMPTY chain
                // yields `env/<name>` (no `env//<name>`) — the byte shape of
                // the old prefix-then-append loop, kept exactly.
                let names = if chain.is_empty() {
                    String::new()
                } else {
                    format!("{}/", chain.join("/"))
                };
                format!(
                    "DeploymentNode://{}/{names}{}",
                    format_name(env),
                    format_name(element.name())
                )
            }
            K::InfrastructureNode => {
                let tail = by_id(element.parent_id()).map_or(String::new(), node_tail);
                format!(
                    "InfrastructureNode://{}/{}",
                    tail,
                    format_name(element.name())
                )
            }
            K::SoftwareSystemInstance => {
                let tail = by_id(element.parent_id()).map_or(String::new(), node_tail);
                let referent = element.referent(self.model());
                format!(
                    "SoftwareSystemInstance://{}/{}[{}]",
                    tail,
                    format_name(referent.name()),
                    element.instance_id().unwrap_or(0)
                )
            }
            K::ContainerInstance => {
                let tail = by_id(element.parent_id()).map_or(String::new(), node_tail);
                let referent = element.referent(self.model());
                let system_name = by_id(referent.parent_id())
                    .and_then(|c| c.system().map(|s| s.item.name.clone()))
                    .unwrap_or_default();
                format!(
                    "ContainerInstance://{}/{}.{}[{}]",
                    tail,
                    format_name(&system_name),
                    format_name(referent.name()),
                    element.instance_id().unwrap_or(0)
                )
            }
        }
    }
}

/// `ViewContent` with the view's full lifetime (`View::content()` ties
/// the borrow to the enum, too short for the engine's id lookups).
fn content_of(view: View<'_>) -> Option<&ViewContent> {
    match view {
        View::Custom(v) => Some(&v.content),
        View::Landscape(v) => Some(&v.content),
        View::Context(v) => Some(&v.content),
        View::Container(v) => Some(&v.content),
        View::Component(v) => Some(&v.content),
        View::Dynamic(v) => Some(&v.content),
        View::Deployment(v) => Some(&v.content),
        View::Filtered(_) | View::Image(_) => None,
    }
}

/// `StringUtils.isNullOrEmpty`: null or TRIM-empty.
#[must_use]
pub(crate) fn is_null_or_empty(s: Option<&str>) -> bool {
    s.is_none_or(|s| s.trim().is_empty())
}

/// The truthy form: `!isNullOrEmpty` (every non-empty gate in the engine).
#[must_use]
pub(crate) fn is_value(s: &str) -> bool {
    !s.trim().is_empty()
}

/// `"true".equalsIgnoreCase(s)`.
#[must_use]
pub(crate) fn is_true_ci(s: &str) -> bool {
    s.eq_ignore_ascii_case("true")
}

/// Java `String.split(Pattern.quote(sep)).length`: no match → 1 (empty
/// input included); trailing empties dropped (all-empty → 0).
#[must_use]
fn java_split_len(s: &str, sep: &str) -> usize {
    if sep.is_empty() || !s.contains(sep) {
        return 1;
    }
    let mut parts: Vec<&str> = s.split(sep).collect();
    while parts.last() == Some(&"") {
        parts.pop();
    }
    parts.len()
}

/// UTF-16 `String.length()`.
#[must_use]
pub(crate) fn utf16_len(s: &str) -> i64 {
    // A UTF-16 length cannot realistically exceed i64::MAX; saturating
    // keeps the comparison well-defined without a panic path.
    i64::try_from(s.encode_utf16().count()).unwrap_or(i64::MAX)
}

/// `ElementView.compareTo`: both ids parse as i32 → numeric, else the id
/// strings (UTF-16 code units).
fn element_view_cmp(a: &ElementView, b: &ElementView) -> std::cmp::Ordering {
    if let (Ok(n1), Ok(n2)) = (a.id.as_str().parse::<i32>(), b.id.as_str().parse::<i32>()) {
        n1.cmp(&n2)
    } else {
        java_cmp_str(a.id.as_str(), b.id.as_str())
    }
}

/// `RelationshipView.compareTo` key: `getId() + "/" + getOrder()` — a
/// `None` order joins as the literal `null`.
fn relationship_view_key(rv: &RelationshipView) -> String {
    // 2R R-exp-04: `as_deref()` feeds `format!` directly — no `order` clone
    // per comparator call; the key stays a `String` (the TreeSet key shape).
    format!("{}/{}", rv.id, rv.order.as_deref().unwrap_or("null"))
}

/// `TreeSet`-style dedup on a sorted vec (keeps the first of each
/// equal-key run — the first ADDITION is what a `TreeSet` keeps).
fn dedup_by_key<T, K: PartialEq, F: Fn(&T) -> K>(items: &mut Vec<T>, key: F) {
    // 2R R-exp-03: `Vec::dedup_by` — same keep-first-of-run semantics as the
    // old index-`remove` walk (which was O(n²) on a run of duplicates).
    items.dedup_by(|a, b| key(a) == key(b));
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidizr_core::Id;
    use oxidizr_core::configuration::Workspace;
    use oxidizr_core::model::{Container, ModelItem, Person, Relationship, SoftwareSystem};
    use oxidizr_core::views::{
        CustomView, DynamicView, ElementView, RelationshipView, SystemContextView,
    };

    /// Writes a token per hook call so tests can assert the EXACT
    /// engine-driver sequence.
    struct MockDialect {
        animation: bool,
        sequence: bool,
        legend: bool,
    }

    impl MockDialect {
        fn new() -> Self {
            Self {
                animation: false,
                sequence: false,
                legend: false,
            }
        }
    }

    impl Dialect for MockDialect {
        fn write_header<'a>(
            &mut self,
            _e: &Engine<'a>,
            _view: View<'a>,
            w: &mut IndentingWriter,
        ) -> Result<(), ExportError> {
            w.write_line(Some("HDR"));
            Ok(())
        }
        fn write_footer<'a>(&mut self, _e: &Engine<'a>, _view: View<'a>, w: &mut IndentingWriter) {
            w.write_line(Some("FTR"));
        }
        fn write_element<'a>(
            &mut self,
            e: &Engine<'a>,
            view: View<'a>,
            element: ElemRef<'a>,
            w: &mut IndentingWriter,
        ) -> Result<(), ExportError> {
            let visible = e.is_visible(view, element).unwrap_or(true);
            let marker = if visible { "E" } else { "x" };
            w.write_line(Some(&format!("{marker} {}", element.id())));
            Ok(())
        }
        fn write_relationship<'a>(
            &mut self,
            _e: &Engine<'a>,
            _view: View<'a>,
            rv: &'a RelationshipView,
            w: &mut IndentingWriter,
        ) -> Result<(), ExportError> {
            w.write_line(Some(&format!("R {}", rv.id)));
            Ok(())
        }
        fn start_group_boundary<'a>(
            &mut self,
            _e: &Engine<'a>,
            _view: View<'a>,
            group: &str,
            w: &mut IndentingWriter,
        ) -> Result<(), ExportError> {
            w.write_line(Some(&format!("G+ {group}")));
            Ok(())
        }
        fn end_group_boundary<'a>(
            &mut self,
            _e: &Engine<'a>,
            _view: View<'a>,
            w: &mut IndentingWriter,
        ) {
            w.write_line(Some("G-"));
        }
        fn start_software_system_boundary<'a>(
            &mut self,
            _e: &Engine<'a>,
            _view: View<'a>,
            system: ElemRef<'a>,
            w: &mut IndentingWriter,
        ) {
            w.write_line(Some(&format!("S+ {}", system.name())));
        }
        fn end_software_system_boundary<'a>(
            &mut self,
            _e: &Engine<'a>,
            _view: View<'a>,
            w: &mut IndentingWriter,
        ) {
            w.write_line(Some("S-"));
        }
        fn start_container_boundary<'a>(
            &mut self,
            _e: &Engine<'a>,
            _view: View<'a>,
            container: ElemRef<'a>,
            w: &mut IndentingWriter,
        ) {
            w.write_line(Some(&format!("C+ {}", container.name())));
        }
        fn end_container_boundary<'a>(
            &mut self,
            _e: &Engine<'a>,
            _view: View<'a>,
            w: &mut IndentingWriter,
        ) {
            w.write_line(Some("C-"));
        }
        fn start_deployment_node_boundary<'a>(
            &mut self,
            _e: &Engine<'a>,
            _view: View<'a>,
            node: ElemRef<'a>,
            w: &mut IndentingWriter,
        ) -> Result<(), ExportError> {
            w.write_line(Some(&format!("N+ {}", node.name())));
            Ok(())
        }
        fn end_deployment_node_boundary<'a>(
            &mut self,
            _e: &Engine<'a>,
            _view: View<'a>,
            w: &mut IndentingWriter,
        ) {
            w.write_line(Some("N-"));
        }
        fn start_boundary<'a>(
            &mut self,
            _e: &Engine<'a>,
            _view: View<'a>,
            element: ElemRef<'a>,
            w: &mut IndentingWriter,
        ) {
            w.write_line(Some(&format!("B+ {}", element.name())));
            w.indent();
        }
        fn end_boundary<'a>(&mut self, _e: &Engine<'a>, _view: View<'a>, w: &mut IndentingWriter) {
            w.outdent();
            w.write_line(Some("end box"));
        }
        fn is_animation_supported<'a>(&self, _e: &Engine<'a>, _view: View<'a>) -> bool {
            self.animation
        }
        fn render_as_sequence_diagram<'a>(&self, _e: &Engine<'a>, _view: View<'a>) -> bool {
            self.sequence
        }
        fn create_diagram(&self, _e: &Engine, view: View<'_>, definition: String) -> Diagram {
            Diagram::new(view.key().to_string(), definition, "puml".to_string())
        }
        fn create_legend(&mut self, _e: &Engine, _view: View<'_>) -> Option<Legend> {
            self.legend.then(|| Legend::new("LEGEND".to_string()))
        }
    }

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
            documentation: oxidizr_core::docs::Documentation::default(),
            relationships: Vec::new(),
        }
    }

    fn container(id: &str, name: &str) -> Container {
        Container {
            item: item(id, name),
            group: None,
            technology: None,
            components: Vec::new(),
            documentation: oxidizr_core::docs::Documentation::default(),
            relationships: Vec::new(),
        }
    }

    fn rel(id: &str, source: &str, destination: &str) -> Relationship {
        Relationship {
            id: Id::new(id),
            source: Id::new(source),
            destination: Id::new(destination),
            description: None,
            technology: None,
            tags: oxidizr_core::tags::Tags::default(),
            url: None,
            properties: oxidizr_core::properties::Properties::default(),
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

    fn rv(id: &str, order: Option<&str>) -> RelationshipView {
        RelationshipView {
            id: Id::new(id),
            description: None,
            url: None,
            order: order.map(str::to_string),
            response: None,
            properties: oxidizr_core::properties::Properties::default(),
            vertices: Vec::new(),
            routing: None,
            jump: None,
            position: None,
        }
    }

    /// Landscape view over persons `10`, `9`, `abc`, `2` (insertion order)
    /// with one relationship: the numeric-id `TreeSet` order, then the
    /// relationship-id sort and the non-empty blank-line legs.
    fn workspace_with_people() -> Workspace {
        let mut workspace = Workspace::default();
        workspace.model.people = vec![
            person("10", "dave"),
            person("9", "carol"),
            person("abc", "xavier"),
            person("2", "bob"),
        ];
        workspace.model.relationships = vec![
            rel("20", "2", "10"),
            rel("3", "9", "abc"),
            rel("100", "abc", "2"),
        ];
        let mut view = SystemContextView {
            base: ViewBase::new("system-landscape", 1),
            content: ViewContent::default(),
            software_system_id: Id::new("1"),
            enterprise_boundary_visible: true,
            animations: Vec::new(),
        };
        view.content.elements = vec![ev("10"), ev("9"), ev("abc"), ev("2")];
        view.content.relationships = vec![rv("20", None), rv("100", None), rv("3", None)];
        workspace.views.system_context_views.push(view);
        workspace
    }

    fn export_first(workspace: &Workspace, d: &mut MockDialect) -> Diagram {
        let mut engine = Engine::new(workspace, ColorScheme::Light);
        engine.export_all(d).expect("export")[0].clone()
    }

    #[test]
    fn element_and_relationship_order_and_blank_legs() {
        let workspace = workspace_with_people();
        let diagram = export_first(&workspace, &mut MockDialect::new());
        // Header, then elements: findGroups re-sorts the list by id STRING
        // in place ("10" < "2" < "9" < "abc"), blank (elements non-empty),
        // relationships sorted by ID STRING (100, 20, 3), blank, footer.
        assert_eq!(
            diagram.definition(),
            "HDR\nE 10\nE 2\nE 9\nE abc\n\nR 100\nR 20\nR 3\n\nFTR"
        );
    }

    #[test]
    fn dynamic_relationship_tree_set_order_and_frames() {
        let mut workspace = Workspace::default();
        workspace.model.people = vec![person("1", "a"), person("2", "b"), person("3", "c")];
        workspace.model.relationships = vec![rel("5", "1", "2"), rel("6", "2", "1")];
        let mut view = DynamicView {
            base: ViewBase::new("dyn", 1),
            content: ViewContent::default(),
            element_id: None,
            deployment_environment: None,
            external_boundaries_visible: false,
        };
        view.content.elements = vec![ev("1"), ev("2"), ev("3")];
        // TreeSet key `id + "/" + order` gives "5/2" < "5/null" < "6/null",
        // but the `DynamicView.getRelationships()` override then re-sorts:
        // orders are NOT all numeric (two are null), so the TEXT branch runs
        // — where the reference NPEs on null orders; oxidizr sorts a missing
        // order first as the empty string (never-crash). Stable ties keep
        // TreeSet order: 5/null, 6/null, then 5/"2".
        view.content.relationships = vec![rv("5", None), rv("6", None), rv("5", Some("2"))];
        workspace.views.dynamic_views.push(view);

        let mut d = MockDialect {
            animation: true,
            sequence: false,
            legend: true,
        };
        let diagrams = {
            let mut engine = Engine::new(&workspace, ColorScheme::Light);
            engine.export_all(&mut d).expect("export")
        };
        let diagram = &diagrams[0];
        // Main export: no frame -> everything visible; the re-sorted order
        // puts the null orders (as "") before the order-"2" view.
        assert_eq!(
            diagram.definition(),
            "HDR\nE 1\nE 2\nE 3\n\nR 5\nR 6\nR 5\n\nFTR"
        );
        // Frames: LinkedHashSet of orders over the RE-SORTED traversal:
        // None, then Some("2") (the second null joins nothing) -> TWO
        // frames; a null order re-exports with frame = null.
        assert_eq!(diagram.frames().len(), 2);
        assert_eq!(diagram.legend().map(Legend::definition), Some("LEGEND"));
        // Frame null (Java null order): identical to the main export.
        assert_eq!(diagram.frames()[0].definition(), diagram.definition());
        // Frame "2": only endpoints of the order-2 relationship are
        // visible; element 3 hides. ALL relationships are still written.
        assert_eq!(
            diagram.frames()[1].definition(),
            "HDR\nE 1\nE 2\nx 3\n\nR 5\nR 6\nR 5\n\nFTR"
        );
    }

    #[test]
    fn nested_group_quirks() {
        let mut workspace = Workspace::default();
        workspace
            .model
            .properties
            .insert("structurizr.groupSeparator", "/");
        let mut a = person("1", "a");
        a.group = Some("x".to_string());
        let mut b = person("2", "b");
        b.group = Some("xy".to_string());
        let mut c = person("3", "c");
        c.group = Some("p/q".to_string());
        workspace.model.people = vec![c, b, a];
        let mut view = SystemContextView {
            base: ViewBase::new("v", 1),
            content: ViewContent::default(),
            software_system_id: Id::new("9"),
            enterprise_boundary_visible: true,
            animations: Vec::new(),
        };
        view.content.elements = vec![ev("1"), ev("2"), ev("3")];
        workspace.views.system_context_views.push(view);

        let diagram = export_first(&workspace, &mut MockDialect::new());
        // groups sorted: ["p", "p/q", "x", "xy"]; context "" -> "p" (1==1,
        // length 0: no close), members none; "p" -> "p/q": deeper, INDENT,
        // E 3; "p/q" -> "x": shallower -> (2-1)x(end AT the current indent
        // +outdent) + one MORE end; "x" -> "xy": EQUAL count but context
        // "x".length()==1 -> NO close (upstream quirk); tail: context "xy"
        // -> one end.
        assert_eq!(
            diagram.definition(),
            "HDR\nG+ p\n  G+ p/q\n  E 3\n  G-\nG-\nG+ x\nE 1\nG+ xy\nE 2\nG-\n\nFTR"
        );
    }

    #[test]
    fn custom_view_cast_abort_and_dynamic_scope_matrix() {
        let mut workspace = Workspace::default();
        workspace.model.people = vec![person("1", "a")];
        let mut custom = CustomView {
            base: ViewBase::new("c", 1),
            content: ViewContent::default(),
            animations: Vec::new(),
            software_system_id: None,
        };
        custom.content.elements = vec![ev("1")];
        workspace.views.custom_views.push(custom);
        let mut engine = Engine::new(&workspace, ColorScheme::Light);
        let err = engine.export_all(&mut MockDialect::new()).unwrap_err();
        assert!(err.to_string().contains("ClassCastException"), "{err}");
    }

    #[test]
    fn animation_frame_visibility_and_legend_lifecycle() {
        let mut workspace = Workspace::default();
        workspace.model.people = vec![person("1", "a"), person("2", "b")];
        let mut view = SystemContextView {
            base: ViewBase::new("v", 1),
            content: ViewContent::default(),
            software_system_id: Id::new("9"),
            enterprise_boundary_visible: true,
            animations: vec![
                Animation {
                    order: 1,
                    element_ids: vec!["1".to_string()],
                    relationship_ids: Vec::new(),
                },
                Animation {
                    order: 2,
                    element_ids: vec!["2".to_string()],
                    relationship_ids: Vec::new(),
                },
            ],
        };
        view.content.elements = vec![ev("1"), ev("2")];
        workspace.views.system_context_views.push(view);
        let mut d = MockDialect {
            animation: true,
            sequence: false,
            legend: true,
        };
        let diagram = export_first(&workspace, &mut d);
        assert_eq!(diagram.definition(), "HDR\nE 1\nE 2\n\nFTR");
        assert_eq!(diagram.frames().len(), 2);
        // Step 1 reveals only element 1; step 2 reveals both (orders <= 2).
        assert_eq!(diagram.frames()[0].definition(), "HDR\nE 1\nx 2\n\nFTR");
        assert_eq!(diagram.frames()[1].definition(), "HDR\nE 1\nE 2\n\nFTR");
        assert_eq!(diagram.legend().map(Legend::definition), Some("LEGEND"));
    }

    #[test]
    fn type_of_terminology_technology_and_symbols() {
        let mut workspace = Workspace::default();
        let mut c = container("2", "web");
        c.technology = Some("Java".to_string());
        let mut sys = system("1", "Big System");
        sys.containers = vec![c];
        workspace.model.software_systems = vec![sys];
        workspace.views.configuration.terminology.container = Some(String::new());
        {
            let engine = Engine::new(&workspace, ColorScheme::Light);
            let sys_ref = element_by_id(&workspace.model, &Id::new("1")).expect("system");
            let container_ref = element_by_id(&workspace.model, &Id::new("2")).expect("container");
            assert_eq!(engine.type_of(sys_ref, true), "[Software System]");
            // Terminology "" falls back to the default; the tech suffix is
            // container-only.
            assert_eq!(engine.type_of(container_ref, true), "[Container: Java]");
            assert_eq!(engine.type_of(container_ref, false), "Container: Java");
        }
        workspace.views.configuration.metadata_symbols = Some(MetadataSymbols::AngleBrackets);
        {
            let engine = Engine::new(&workspace, ColorScheme::Light);
            let container_ref = element_by_id(&workspace.model, &Id::new("2")).expect("container");
            assert_eq!(engine.type_of(container_ref, true), "<Container: Java>");
        }
    }

    #[test]
    fn break_text_wraps_with_java_separators() {
        let workspace = Workspace::default();
        let engine = Engine::new(&workspace, ColorScheme::Light);
        // maxCharacters = (int)(100 / (10 x 0.6)) = 16.
        assert_eq!(engine.break_text(100, 10, "short"), "short");
        assert_eq!(
            engine.break_text(100, 10, "aaaa bbbb cccc dddd eeee"),
            "aaaa bbbb cccc<br />dddd eeee"
        );
        assert_eq!(engine.break_text(100, 10, "   "), "");
    }

    #[test]
    fn sequence_path_boundary_stack_and_sibling_insertion() {
        let mut workspace = Workspace::default();
        let mut c1 = container("11", "api");
        c1.relationships = vec![Id::new("r1")];
        let mut c2 = container("12", "db");
        c2.relationships = vec![Id::new("r2")];
        let mut sys = system("1", "Sys");
        sys.containers = vec![c1, c2];
        workspace.model.software_systems = vec![sys];
        workspace.model.relationships = vec![
            rel("r1", "1", "11"),
            rel("r2", "11", "12"),
            rel("r3", "11", "12"),
        ];
        // dyn1: the PARENT system is itself in the element set -> no box
        // is ever opened (the reference only boxes absent parents).
        let mut dyn1 = DynamicView {
            base: ViewBase::new("dyn1", 1),
            content: ViewContent::default(),
            element_id: None,
            deployment_environment: None,
            external_boundaries_visible: false,
        };
        dyn1.content.relationships = vec![rv("r1", Some("1")), rv("r2", Some("2"))];
        // dyn2: only containers -> the owning system gets a boundary box.
        let mut dyn2 = DynamicView {
            base: ViewBase::new("dyn2", 2),
            content: ViewContent::default(),
            element_id: None,
            deployment_environment: None,
            external_boundaries_visible: false,
        };
        dyn2.content.relationships = vec![rv("r3", Some("1"))];
        workspace.views.dynamic_views = vec![dyn1, dyn2];
        workspace
            .views
            .configuration
            .properties
            .insert("plantuml.sequenceDiagram", "true");

        let mut d = MockDialect {
            animation: true,
            sequence: true,
            legend: true,
        };
        let diagrams = {
            let mut engine = Engine::new(&workspace, ColorScheme::Light);
            engine.export_all(&mut d).expect("export")
        };
        // Sequence renders get NO frames (the Java override returns before
        // the frame loop) but DO get the legend (created after the body).
        assert_eq!(diagrams[0].frames(), []);
        assert_eq!(diagrams[0].legend().map(Legend::definition), Some("LEGEND"));
        // Element set 1, 11, 12; write order 1, 11, 12 (11 after its
        // parent, 12 after its sibling); all in-set -> no boxes.
        assert_eq!(
            diagrams[0].definition(),
            "HDR\nE 1\nE 11\nE 12\n\nR r1\nR r2\n\nFTR"
        );
        // dyn2: system 1 is ABSENT from the element set -> boundary box
        // around both containers (siblings insert adjacent, inside).
        assert_eq!(
            diagrams[1].definition(),
            "HDR\nB+ Sys\n  E 11\n  E 12\nend box\n\nR r3\n\nFTR"
        );
    }
}
