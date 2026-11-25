//! The dialect seam of the shared diagram engine: the `Frame` slot and the
//! `Dialect` trait of the `write*`/`start*`/`end*` hooks the [`Engine`]
//! drives. The Structurizr and C4 dialects implement this; the engine
//! re-exports both names, so `crate::plantuml::engine::{Dialect, Frame}`
//! stays the stable path.

use oxidizr_core::views::{DynamicView, RelationshipView, View};

use crate::diagram::{Diagram, Legend};
use crate::error::ExportError;
use crate::indent::IndentingWriter;
use crate::plantuml::model::ElemRef;

use super::Engine;

/// The `frame` field — the official untyped slot: an integer (static/
/// deployment animation step, also used for custom views) or a string
/// (dynamic relationship-view order). A null order stays `null`
/// (the `LinkedHashSet` keeps the null entry; a null frame write stays
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
/// The Structurizr and C4 dialects implement this.
/// Every hook ties the engine, view and element to ONE lifetime `'a`, so
/// implementations can call the engine's element helpers
/// (`type_of`/`canonical_name`/`property`) with the element they are
/// handed.
pub trait Dialect {
    /// `writeHeader`. `Result` because the Structurizr header's title
    /// fallback (the view name) crashes on a dangling view scope.
    ///
    /// # Errors
    ///
    /// [`ExportError::ReferenceAbort`] on the official tooling null-pointer/cast paths reached
    /// while writing the header (e.g. a dangling view scope).
    fn write_header<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError>;
    /// `writeFooter`.
    fn write_footer<'a>(&mut self, e: &Engine<'a>, view: View<'a>, w: &mut IndentingWriter);
    /// `writeElement`. `Result` because it consults `isVisible` (the official tooling
    /// untyped-`frame` cast) — the `hide <id>` leg.
    ///
    /// # Errors
    ///
    /// [`ExportError::ReferenceAbort`] on the official tooling null-pointer/cast paths reached
    /// while writing the element (e.g. the frame-mismatch cast).
    fn write_element<'a>(
        &mut self,
        e: &Engine<'a>,
        view: View<'a>,
        element: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError>;
    /// `writeRelationship`. `Result` because the model relationship behind
    /// the view must resolve (the official tooling would null-pointer crash).
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
    /// group-separator cut is an index-abort path.
    ///
    /// # Errors
    ///
    /// [`ExportError::ReferenceAbort`] when the group-separator cut falls
    /// outside the group name (the bounds check on the cut).
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
    /// [`ExportError::ReferenceAbort`] on the official tooling null-pointer/cast paths reached
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
    /// catalog `EXPORT-MERMAID.md`  — emits a standalone document with
    /// its own grammar and NO header/footer/legend/frames. When this is
    /// `true`, `render_as_sequence_diagram` routes to
    /// [`Dialect::sequence_document`] instead .
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
    /// [`ExportError::ReferenceAbort`] on the official tooling null-pointer crash paths of the owned
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
    /// Whether the dialect emits `CustomView` diagrams at all: the official tooling
    /// `C4PlantUMLExporter.export(CustomView)` overload returns `null`, and
    /// the workspace export loop then skips the view entirely (verified:
    /// verified: the `custom-view-animation` C4 sweep writes NO files).
    fn exports_custom_views(&self) -> bool {
        true
    }
    /// `createDiagram`.
    fn create_diagram(&self, e: &Engine, view: View<'_>, definition: String) -> Diagram;
    /// `createLegend` (base returns `null`). Called AFTER the last body
    /// export, with the frame state of that export.
    fn create_legend(&mut self, _e: &Engine, _view: View<'_>) -> Option<Legend> {
        None
    }
}
