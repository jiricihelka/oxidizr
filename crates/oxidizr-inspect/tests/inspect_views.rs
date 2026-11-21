//! §P4-M1 I-3 view-phase tests: the `runViewInspections` order snapshot
//! plus the view-side severity holder chains (view → view-config →
//! workspace, style → view-config → workspace, views → view-config →
//! workspace). Re-bless: `UPDATE_EXPECT=1 cargo test -p oxidizr-inspect`.
#![allow(clippy::unwrap_used)]

use expect_test::expect;
use oxidizr_core::configuration::{Workspace, WorkspaceConfiguration, WorkspaceScope};
use oxidizr_core::ids::Id;
use oxidizr_core::views::{
    AutomaticLayout, ComponentView, ContainerView, CustomView, DeploymentView, DynamicView,
    ElementStyle, ElementView, FilterMode, FilteredView, ImageView, SystemContextView,
    SystemLandscapeView, ViewBase, ViewContent,
};
use oxidizr_inspect::{Severity, Violation, inspect};

// -- fixture builders -----------------------------------------------------

fn base(key: &str, order: i32, generated: bool) -> ViewBase {
    let mut b = ViewBase::new(key, order);
    b.generated_key = generated;
    b
}

fn content(elements: &[&str], layout: Option<AutomaticLayout>) -> ViewContent {
    ViewContent {
        elements: elements
            .iter()
            .map(|e| ElementView {
                id: Id::new(*e),
                x: 0,
                y: 0,
            })
            .collect(),
        relationships: Vec::new(),
        paper_size: None,
        dimensions: None,
        automatic_layout: layout,
    }
}

fn custom(key: &str, order: i32, generated: bool, elements: &[&str]) -> CustomView {
    CustomView {
        base: base(key, order, generated),
        content: content(elements, None),
        animations: Vec::new(),
        software_system_id: None,
    }
}

fn landscape(key: &str, order: i32, elements: &[&str]) -> SystemLandscapeView {
    SystemLandscapeView {
        base: base(key, order, false),
        content: content(elements, None),
        enterprise_boundary_visible: false,
        animations: Vec::new(),
    }
}

fn context(
    key: &str,
    order: i32,
    generated: bool,
    system: &str,
    elements: &[&str],
) -> SystemContextView {
    SystemContextView {
        base: base(key, order, generated),
        content: content(elements, None),
        software_system_id: Id::new(system),
        enterprise_boundary_visible: false,
        animations: Vec::new(),
    }
}

fn container(
    key: &str,
    order: i32,
    generated: bool,
    system: &str,
    elements: &[&str],
) -> ContainerView {
    ContainerView {
        base: base(key, order, generated),
        content: content(elements, None),
        software_system_id: Id::new(system),
        external_software_system_boundaries_visible: false,
        animations: Vec::new(),
    }
}

fn component(key: &str, order: i32, elements: &[&str]) -> ComponentView {
    ComponentView {
        base: base(key, order, true),
        content: content(elements, None),
        container_id: Id::new("20"),
        external_container_boundaries_visible: false,
        animations: Vec::new(),
    }
}

fn dynamic(key: &str, order: i32, elements: &[&str]) -> DynamicView {
    DynamicView {
        base: base(key, order, true),
        content: content(elements, None),
        element_id: None,
        deployment_environment: None,
        external_boundaries_visible: false,
    }
}

fn deployment(key: &str, order: i32, elements: &[&str]) -> DeploymentView {
    DeploymentView {
        base: base(key, order, true),
        content: content(elements, Some(AutomaticLayout::default())),
        environment: "Default".into(),
        software_system_id: None,
        animations: Vec::new(),
    }
}

fn filtered(key: &str, order: i32, generated: bool) -> FilteredView {
    FilteredView {
        base: base(key, order, generated),
        base_view_key: "Gen-1".into(),
        mode: Some(FilterMode::Include),
        tags: vec!["Person".into()],
    }
}

fn image(key: &str, order: i32, generated: bool) -> ImageView {
    ImageView {
        base: base(key, order, generated),
        element_id: None,
        content: None,
        content_light: None,
        content_dark: None,
        content_type: None,
    }
}

fn style(tag: &str, metadata: Option<bool>) -> ElementStyle {
    ElementStyle {
        tag: tag.into(),
        metadata,
        ..ElementStyle::default()
    }
}

/// A workspace whose model phase fires exactly `model.empty` (scope is
/// set, so `workspace.scope` is silenced) and whose views exercise EVERY
/// view-phase position — each view kind, generated keys, empty and
/// non-empty content, the layout pairing, and the two multi-system scope
/// checks.
fn view_kitchen_sink() -> Workspace {
    let views = oxidizr_core::views::ViewSet {
        custom_views: vec![custom("Gen-1", 1, true, &[])],
        system_landscape_views: vec![landscape("Land-1", 2, &[])],
        system_context_views: vec![
            context("SC-1", 3, false, "11", &["2"]),
            context("Gen-SC", 4, true, "22", &["2"]),
        ],
        container_views: vec![
            container("CV-1", 5, false, "11", &["2"]),
            container("CV-2", 6, false, "33", &[]),
        ],
        component_views: vec![component("Comp-1", 7, &["2"])],
        dynamic_views: vec![dynamic("Dyn-1", 8, &[])],
        deployment_views: vec![deployment("Dep-1", 9, &["2"])],
        filtered_views: vec![filtered("Gen-F", 10, true), filtered("F-2", 11, false)],
        image_views: vec![image("Gen-I", 12, true)],
        configuration: oxidizr_core::views::ViewConfiguration {
            styles: oxidizr_core::views::Styles {
                element_styles: vec![
                    style("Person", Some(false)),
                    style("System", Some(true)),
                    style("X", None),
                ],
                ..oxidizr_core::views::Styles::default()
            },
            ..oxidizr_core::views::ViewConfiguration::default()
        },
    };
    Workspace {
        configuration: WorkspaceConfiguration {
            scope: Some(WorkspaceScope::Landscape),
            ..WorkspaceConfiguration::default()
        },
        views,
        ..Workspace::default()
    }
}

/// Snapshot format: one `type | SEVERITY | message` line per violation
/// (the reference `toString`).
fn snap(violations: &[Violation]) -> String {
    let mut out = String::new();
    for v in violations {
        out.push_str(&v.to_string());
        out.push('\n');
    }
    out
}

fn severities(violations: &[Violation]) -> Vec<(&str, Severity)> {
    violations
        .iter()
        .map(|v| (v.inspection, v.severity))
        .collect()
}

// -- the run-order snapshot -----------------------------------------------

#[test]
fn view_phase_run_order_snapshot() {
    // Every view-phase position fires in the `runViewInspections` order:
    // views.empty (absent — filtered/image views count), the per-kind
    // model-view loops with the multi-system checks woven in, the
    // generated-key-only filtered/image loops, then the element styles.
    let violations = inspect(&view_kitchen_sink());
    expect![[r#"
        model.empty | ERROR | The model is empty.
        views.view.key | ERROR | The view with key "Gen-1" has an automatically generated view key and this is not guaranteed to be stable over time.
        views.view.empty | ERROR | The view with key "Gen-1" is empty.
        views.view.layout | ERROR | The view with key "Gen-1" has an automatically generated view key and this may cause manual layout information to be lost in the future.
        views.view.empty | ERROR | The view with key "Land-1" is empty.
        workspace.scope | ERROR | System context views exist for 2 software systems. It is recommended that a workspace includes system context views for a single software system only.
        views.view.key | ERROR | The view with key "Gen-SC" has an automatically generated view key and this is not guaranteed to be stable over time.
        views.view.layout | ERROR | The view with key "Gen-SC" has an automatically generated view key and this may cause manual layout information to be lost in the future.
        workspace.scope | ERROR | Container views exist for 2 software systems. It is recommended that a workspace includes container views for a single software system only.
        views.view.empty | ERROR | The view with key "CV-2" is empty.
        views.view.key | ERROR | The view with key "Comp-1" has an automatically generated view key and this is not guaranteed to be stable over time.
        views.view.layout | ERROR | The view with key "Comp-1" has an automatically generated view key and this may cause manual layout information to be lost in the future.
        views.view.key | ERROR | The view with key "Dyn-1" has an automatically generated view key and this is not guaranteed to be stable over time.
        views.view.empty | ERROR | The view with key "Dyn-1" is empty.
        views.view.layout | ERROR | The view with key "Dyn-1" has an automatically generated view key and this may cause manual layout information to be lost in the future.
        views.view.key | ERROR | The view with key "Dep-1" has an automatically generated view key and this is not guaranteed to be stable over time.
        views.view.key | ERROR | The view with key "Gen-F" has an automatically generated view key and this is not guaranteed to be stable over time.
        views.view.key | ERROR | The view with key "Gen-I" has an automatically generated view key and this is not guaranteed to be stable over time.
        views.styles.element.metadata | ERROR | The element style for tag "Person" has metadata hidden, which may introduce ambiguity on rendered diagrams.
    "#]]
    .assert_eq(&snap(&violations));
}

#[test]
fn filtered_or_image_views_alone_still_count_as_views() {
    // `ViewSet.isEmpty()` covers ALL nine kinds — an image-view-only
    // workspace must NOT fire views.empty.
    let views = oxidizr_core::views::ViewSet {
        image_views: vec![image("I", 1, false)],
        ..oxidizr_core::views::ViewSet::default()
    };
    let ws = Workspace {
        configuration: WorkspaceConfiguration {
            scope: Some(WorkspaceScope::Landscape),
            ..WorkspaceConfiguration::default()
        },
        views,
        ..Workspace::default()
    };
    let kinds: Vec<&str> = inspect(&ws).iter().map(|v| v.inspection).collect();
    assert_eq!(kinds, ["model.empty"]);
}

// -- the view-side severity holder chains ---------------------------------

#[test]
fn view_severity_holder_chain() {
    // View → view-config → workspace, longest-first with the `*`
    // fallback, first holder hit wins:
    // - key:     exact match on the VIEW's own properties (warning),
    // - empty:   exact match on the view-CONFIG (info),
    // - layout:  NO exact match anywhere — the workspace-level
    //            `structurizr.inspection.views.*` wildcard catches it
    //            (info),
    // - views.empty: the view-config exact match (warning) shadows the
    //            workspace wildcard (info),
    // - style A: exact match on the STYLE's properties (warning),
    // - style B: no exact match — the workspace wildcard (info).
    let mut ws = Workspace {
        configuration: WorkspaceConfiguration {
            scope: Some(WorkspaceScope::Landscape),
            ..WorkspaceConfiguration::default()
        },
        ..Workspace::default()
    };
    ws.properties
        .insert("structurizr.inspection.views.*", "info");
    ws.views
        .configuration
        .properties
        .insert("structurizr.inspection.views.empty", "warning");
    ws.views
        .configuration
        .properties
        .insert("structurizr.inspection.views.view.empty", "info");

    let mut v = custom("Gen-1", 1, true, &[]);
    v.base
        .properties
        .insert("structurizr.inspection.views.view.key", "warning");
    ws.views.custom_views = vec![v];

    let mut hidden = style("Person", Some(false));
    hidden.properties.insert(
        "structurizr.inspection.views.styles.element.metadata",
        "warning",
    );
    ws.views.configuration.styles.element_styles = vec![hidden, style("System", Some(false))];

    let violations = inspect(&ws);
    assert_eq!(
        severities(&violations),
        [
            ("model.empty", Severity::Error),
            ("views.view.key", Severity::Warning),
            ("views.view.empty", Severity::Info),
            ("views.view.layout", Severity::Info),
            ("views.styles.element.metadata", Severity::Warning,), // style A
            ("views.styles.element.metadata", Severity::Info,),    // style B — wildcard
        ]
    );
}

#[test]
fn inner_holder_exact_match_wins_over_outer() {
    // The chain is inner-first: a VIEW property beats the view-config
    // property for the same exact key.
    let mut ws = Workspace {
        configuration: WorkspaceConfiguration {
            scope: Some(WorkspaceScope::Landscape),
            ..WorkspaceConfiguration::default()
        },
        ..Workspace::default()
    };
    ws.views
        .configuration
        .properties
        .insert("structurizr.inspection.views.view.key", "error");
    let mut v = custom("Gen-1", 1, true, &["2"]);
    v.base
        .properties
        .insert("structurizr.inspection.views.view.key", "ignore");
    ws.views.custom_views = vec![v];
    // (the layout violation still fires at its default ERROR — the
    // override targets the key type only)
    assert_eq!(
        severities(&inspect(&ws)),
        [
            ("model.empty", Severity::Error),
            ("views.view.key", Severity::Ignore),
            ("views.view.layout", Severity::Error),
        ]
    );
}
