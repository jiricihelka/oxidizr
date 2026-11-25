//! The shared diagram engine — the port of `export/AbstractDiagramExporter`
//! (848 lines) + the `AbstractExporter` helpers (`breakText`, `typeOf`),
//! pinned. Iteration contracts: the export catalog.
//!
//! The official base class is abstract over the dialect’s writers; the Rust
//! shape is an [`Engine`] struct (workspace + color scheme + the untyped
//! `frame` state) driving a [`Dialect`] trait of the `write*`/`start*`/
//! `end*` hooks. Element "identity" (the official tooling never overrides `equals` — every
//! `contains`/`indexOf`/`Set` is identity-based) becomes id equality: the
//! model keeps one object per id.

use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;

use oxidizr_core::configuration::Workspace;
use oxidizr_core::model::{Model, Relationship};
use oxidizr_core::views::{
    Animation, ColorScheme, ElementView, RelationshipView, View, ViewBase, ViewContent,
};

use crate::diagram::Diagram;
use crate::error::ExportError;
use crate::plantuml::java_cmp_str;
use crate::property::export_property;
mod dialect;

pub use dialect::{Dialect, Frame};
mod query;
mod write;

/// The shared engine state (workspace, color scheme, animation frame).
pub struct Engine<'a> {
    workspace: &'a Workspace,
    color_scheme: ColorScheme,
    frame: Option<Frame>,
    /// lazy id → relationship index: the official tooling’s
    /// id lookup scans the relationship list per call; oxidizr
    /// builds the map once per export (first id wins — the `find` semantics
    /// of the old scan on a duplicate id).
    rel_index: OnceCell<HashMap<&'a str, &'a Relationship>>,
    /// memo for `canonical_name` (id → name; the workspace is
    /// immutable for `'a`, so the name is a pure function of the id).
    name_index: RefCell<HashMap<&'a str, String>>,
}

impl<'a> Engine<'a> {
    /// New engine over `workspace` (frame state starts empty).
    // no longer `const` — the memo maps need `HashMap::new()`
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
    // pub-in-crate shrink — zero callers
    // outside this crate; the getter stays as the official parity surface
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
    // zero callers outside this crate;
    // stays as the parity surface for the frame-field read.
    #[allow(dead_code)] // Java parity surface, see above
    #[must_use]
    pub(crate) const fn frame(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }

    /// The view-or-viewset property lookup — the two-level cascade.
    #[must_use]
    pub fn property(&self, view: &'a ViewBase, name: &str, default: &'a str) -> &'a str {
        export_property(view, &self.workspace.views, name, default)
    }

    /// `export(Workspace)`: custom → landscape → context → container →
    /// component → dynamic → deployment views, each in stored order; image
    /// and filtered views are NOT exported (the official tooling loop skips them).
    ///
    /// # Errors
    ///
    /// [`ExportError::ReferenceAbort`] when a view aborts the export (the
    /// the official tooling cast/null-pointer crash paths; [`ExportError::Json`] never fires here.
    pub fn export_all<D: Dialect>(&mut self, d: &mut D) -> Result<Vec<Diagram>, ExportError> {
        let mut diagrams = Vec::new();
        for view in self.workspace.views.all_views() {
            if matches!(view, View::Filtered(_) | View::Image(_)) {
                continue;
            }
            // The official overload returning `null` (C4 custom views) makes
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
    /// (the official cast/null-pointer crash paths).
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
            // iterate directly — `animations` borrows the
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

/// The official blank check: null or TRIM-empty.
#[must_use]
pub(crate) fn is_null_or_empty(s: Option<&str>) -> bool {
    s.is_none_or(|s| s.trim().is_empty())
}

/// The truthy form (every non-empty gate in the engine).
#[must_use]
pub(crate) fn is_value(s: &str) -> bool {
    !s.trim().is_empty()
}

/// `"true"`, compared case-insensitively.
#[must_use]
pub(crate) fn is_true_ci(s: &str) -> bool {
    s.eq_ignore_ascii_case("true")
}

/// The official separator count: no match → 1 (empty
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
    // `as_deref()` feeds `format!` directly — no `order` clone
    // per comparator call; the key stays a `String` (the TreeSet key shape).
    format!("{}/{}", rv.id, rv.order.as_deref().unwrap_or("null"))
}

/// `TreeSet`-style dedup on a sorted vec (keeps the first of each
/// equal-key run — the first ADDITION is what a `TreeSet` keeps).
fn dedup_by_key<T, K: PartialEq, F: Fn(&T) -> K>(items: &mut Vec<T>, key: F) {
    // `Vec::dedup_by` — same keep-first-of-run semantics as the
    // old index-`remove` walk (which was O(n²) on a run of duplicates).
    items.dedup_by(|a, b| key(a) == key(b));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::Legend;
    use crate::indent::IndentingWriter;
    use crate::plantuml::model::{ElemRef, element_by_id};
    use oxidizr_core::Id;
    use oxidizr_core::configuration::Workspace;
    use oxidizr_core::model::{Container, ModelItem, Person, Relationship, SoftwareSystem};
    use oxidizr_core::views::{
        CustomView, DynamicView, ElementView, MetadataSymbols, RelationshipView, SystemContextView,
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
        // — where the official tooling crashes on null orders; oxidizr sorts a missing
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
        // Frame null (missing order): identical to the main export.
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
        // is ever opened (the official tooling only boxes absent parents).
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
        // Sequence renders get NO frames (the official tooling override returns before
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
