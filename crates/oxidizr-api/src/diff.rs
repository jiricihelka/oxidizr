//! `diff` — workspace-aware model diff.
//!
//! The ENGINE is pure data: two loaded [`Workspace`]s in, a [`Diff`] out —
//! no I/O, no formatting (the schema-1 JSON and the human report live
//! in the command bodies). The identity model never matches on ids — those are
//! unstable across edits: elements are keyed by their canonical name
//! (the `Person://X` URI prefix IS the type), relationships by
//! [`WorkspaceBuilder::relationship_canonical_name`]
//! (`Relationship://A -> B (description)`), views by key, styles by
//! (kind, tag). SAME identity with a different id is its own category
//! ([`Diff::id_changed`]), never add+remove.
//!
//! Locked-in consequences (decided in the phase plan,
//! documented for `docs/diff-schema.md`): a relationship
//! DESCRIPTION or an element NAME change IS an identity change (both are
//! embedded in the canonical-name key ⇒ add+remove pair). A duplicate
//! canonical name (an invalid workspace — validation flags it) is matched
//! positionally: the Nth occurrence pairs with the Nth.
//!
//! Every comparison is field-level ([`Change`]s with JSON `before`/`after`
//! values); free-form properties diff PER KEY as `properties.<key>`.
//! View GEOMETRY (`position[…]`, `vertices[…]`, `automaticLayout`,
//! `dimensions`, `animations`) is gated by [`DiffOptions::ignore_layout`].
//! Image-view content blobs are never compared (base64 payloads).
//! Loader-representation rule (D-4): comparison goes through the official tooling
//! EFFECTIVE views — tags via `getTags` (defaults ++ stored, deduped),
//! empty text reads as ABSENT, and the loader-regenerated `structurizr.dsl*`
//! provenance stamps are NEVER compared — so a `.dsl` and its JSON twin
//! diff EMPTY (pinned by `tests/diff_probes.rs`).
//! Output is deterministic: every category is sorted before returning.

use std::collections::BTreeMap;

use oxidizr_core::Workspace;
use oxidizr_core::builder::WorkspaceBuilder;
use oxidizr_core::ids::Id;
use oxidizr_core::model::{DeploymentNode, Model, ModelItem, Relationship, SoftwareSystem};
use oxidizr_core::properties::Properties;
use oxidizr_core::tags::{TagKind, relationship_defaults};
use oxidizr_core::views::{
    Animation, AutomaticLayout, Dimensions, ElementStyle, RelationshipStyle, View, ViewContent,
};
use serde_json::{Value, json};

/// Diff knobs (: `--ignore-layout` today; extension by addition).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiffOptions {
    /// Skip view geometry fields (element positions, relationship
    /// vertices, automatic layout, dimensions, animation steps).
    pub ignore_layout: bool,
}

/// One identity that exists on exactly one side (schema `added`/`removed`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Dotted canonical path (`model.softwareSystems[Payment System]`).
    pub path: String,
    /// Item kind (`person`, `softwareSystem`, `relationship`, `view`, …).
    pub kind: &'static str,
}

/// One changed field of a matched pair (schema `changed`).
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    /// Dotted canonical path of the item.
    pub path: String,
    /// Changed field (JSON-ish name; `properties.<key>` per property).
    pub field: String,
    /// Value on side A (absent ⇒ `Value::Null`).
    pub before: Value,
    /// Value on side B (absent ⇒ `Value::Null`).
    pub after: Value,
}

/// Same identity, different id (schema `idChanged`) — informational only;
/// the pair still gets its field-level [`Change`]s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdChange {
    /// Dotted canonical path of the item.
    pub path: String,
    /// Id on side A.
    pub before: String,
    /// Id on side B.
    pub after: String,
}

/// The whole comparison, every category sorted deterministically.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Diff {
    /// Identities only on side B.
    pub added: Vec<Entry>,
    /// Identities only on side A.
    pub removed: Vec<Entry>,
    /// Field-level changes of matched pairs.
    pub changed: Vec<Change>,
    /// Matched pairs whose id drifted.
    pub id_changed: Vec<IdChange>,
}

impl Diff {
    /// Nothing to report — the workspaces are equal under the identity
    /// model (the `diff(a, a) == empty` property asserts this on `a == a`).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.changed.is_empty()
            && self.id_changed.is_empty()
    }

    /// The human report: ± entries grouped by category, in
    /// schema order (`added`, `removed`, `changed`, `idChanged`). The
    /// rendering lives here so the CLI stays a replay shim (the bodies-in-API
    /// invariant); ANSI paint rides the `color` flag (the CLI fills it from
    /// `!no_color && NO_COLOR unset`, so API/e2e runs stay PLAIN).
    #[must_use]
    pub fn human_lines(&self, color: bool) -> Vec<String> {
        let paint = |code: &str, text: &str| {
            if color {
                format!("\x1b[{code}m{text}\x1b[0m")
            } else {
                text.to_owned()
            }
        };
        let mut out = Vec::new();
        for (title, entries, sign, code) in [
            ("added", &self.added, "+", "32"),
            ("removed", &self.removed, "-", "31"),
        ] {
            if entries.is_empty() {
                continue;
            }
            out.push(format!("{title} ({}):", entries.len()));
            for e in entries {
                out.push(format!("{} {} ({})", paint(code, sign), e.path, e.kind));
            }
        }
        if !self.changed.is_empty() {
            out.push(format!("changed ({}):", self.changed.len()));
            for c in &self.changed {
                let before = c.before.to_string();
                let after = c.after.to_string();
                out.push(format!(
                    "{} {} {}: {} -> {}",
                    paint("33", "~"),
                    c.path,
                    c.field,
                    before,
                    after
                ));
            }
        }
        if !self.id_changed.is_empty() {
            out.push(format!("idChanged ({}):", self.id_changed.len()));
            for c in &self.id_changed {
                let before = &c.before;
                let after = &c.after;
                out.push(format!(
                    "{} {} id: {before} -> {after}",
                    paint("36", "#"),
                    c.path
                ));
            }
        }
        out
    }

    /// The frozen SCHEMA 1 document (`docs/diff-schema.md`):
    /// `{schema:1, added:[{path,kind}], removed:[…], changed:[{path,
    /// field, before, after}], idChanged:[{path, before, after}]}`.
    /// All four categories are ALWAYS present (empty arrays included)
    /// and the key order is the documented one — `serde_json`'s
    /// `preserve_order` keeps map insertion order, so the serialization
    /// of a given `Diff` is a stable byte string. Absent field values
    /// are `null` (the engine's uniform absent convention).
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "schema": 1,
            "added": self
                .added
                .iter()
                .map(|e| json!({ "path": e.path, "kind": e.kind }))
                .collect::<Vec<Value>>(),
            "removed": self
                .removed
                .iter()
                .map(|e| json!({ "path": e.path, "kind": e.kind }))
                .collect::<Vec<Value>>(),
            "changed": self
                .changed
                .iter()
                .map(|c| {
                    json!({
                        "path": c.path,
                        "field": c.field,
                        "before": c.before,
                        "after": c.after,
                    })
                })
                .collect::<Vec<Value>>(),
            "idChanged": self
                .id_changed
                .iter()
                .map(|c| json!({ "path": c.path, "before": c.before, "after": c.after }))
                .collect::<Vec<Value>>(),
        })
    }
}

/// Compare two loaded workspaces (both sides already through the
/// standard pipeline — this function never loads or validates).
#[must_use]
pub fn diff(a: &Workspace, b: &Workspace, opts: &DiffOptions) -> Diff {
    let ia = index(a, *opts);
    let ib = index(b, *opts);
    let mut out = Diff::default();
    for (key, item) in &ia {
        match ib.get(key) {
            None => out.removed.push(Entry {
                path: item.path.clone(),
                kind: item.kind,
            }),
            Some(other) => {
                if let (Some(id_a), Some(id_b)) = (&item.id, &other.id)
                    && id_a != id_b
                {
                    out.id_changed.push(IdChange {
                        path: item.path.clone(),
                        before: id_a.as_str().to_owned(),
                        after: id_b.as_str().to_owned(),
                    });
                }
                compare_fields(&item.path, &item.fields, &other.fields, &mut out.changed);
            }
        }
    }
    for (key, item) in &ib {
        if !ia.contains_key(key) {
            out.added.push(Entry {
                path: item.path.clone(),
                kind: item.kind,
            });
        }
    }
    // Workspace-level metadata (never added/removed, only changed).
    let fa = workspace_fields(a);
    let fb = workspace_fields(b);
    compare_fields("workspace", &fa, &fb, &mut out.changed);
    out.added
        .sort_by(|x, y| x.path.cmp(&y.path).then_with(|| x.kind.cmp(y.kind)));
    out.removed
        .sort_by(|x, y| x.path.cmp(&y.path).then_with(|| x.kind.cmp(y.kind)));
    out.changed
        .sort_by(|x, y| x.path.cmp(&y.path).then_with(|| x.field.cmp(&y.field)));
    out.id_changed.sort_by(|x, y| x.path.cmp(&y.path));
    out
}

// ---------------------------------------------------------------------------
// The indexed side
// ---------------------------------------------------------------------------

/// Field name → comparable value. Absent keys compare as `Value::Null`.
type Fields = BTreeMap<String, Value>;

/// One indexed item of one side: where it is, what it is, its fields.
struct Item {
    path: String,
    kind: &'static str,
    /// Model items carry their (unstable) id; views/styles don't.
    id: Option<Id>,
    fields: Fields,
}

/// Identity key → item. Duplicate identity keys (invalid workspaces) get
/// a positional `#N` suffix so the Nth occurrence pairs with the Nth.
type Index = BTreeMap<String, Item>;

/// Insert `item` under `key`, positionally disambiguating duplicates.
fn insert_item(index: &mut Index, key: String, item: Item) {
    let mut candidate = key;
    let mut n = 2;
    while index.contains_key(&candidate) {
        candidate = format!("{candidate}#{n}");
        n += 1;
    }
    index.insert(candidate, item);
}

/// Build the identity index of one side (canonical-name machinery over an
/// adopted clone — the reconcile precedent).
fn index(ws: &Workspace, opts: DiffOptions) -> Index {
    let mut b = WorkspaceBuilder::new();
    b.adopt(ws.clone());
    let canon = |id: &Id| {
        b.canonical_name_full(id)
            .unwrap_or_else(|| format!("Unresolved://{id}"))
    };
    let rel_canon = |id: &Id| {
        b.relationship_canonical_name(id)
            .unwrap_or_else(|| format!("Relationship://#{id}"))
    };
    let mut index = Index::new();
    index_model(&ws.model, &canon, &rel_canon, &mut index);
    index_views(ws, &canon, &rel_canon, opts, &mut index);
    index_styles(ws, &mut index);
    index
}

/// The `Scheme://tail` bracket part of a dotted path.
fn bracket(canonical: &str) -> String {
    canonical
        .split_once("://")
        .map_or_else(|| canonical.to_owned(), |(_, tail)| tail.to_owned())
}

fn index_model(
    model: &Model,
    canon: &dyn Fn(&Id) -> String,
    rel_canon: &dyn Fn(&Id) -> String,
    index: &mut Index,
) {
    for ce in &model.custom_elements {
        let key = canon(&ce.item.id);
        let tail = bracket(&key);
        let mut fields = base_fields(&ce.item, TagKind::CustomElement);
        push(&mut fields, "group", opt(ce.group.as_ref()));
        insert_item(
            index,
            key,
            Item {
                path: format!("model.customElements[{tail}]"),
                kind: "customElement",
                id: Some(ce.item.id.clone()),
                fields,
            },
        );
    }
    for p in &model.people {
        let key = canon(&p.item.id);
        let tail = bracket(&key);
        let mut fields = base_fields(&p.item, TagKind::Person);
        push(&mut fields, "group", opt(p.group.as_ref()));
        insert_item(
            index,
            key,
            Item {
                path: format!("model.people[{tail}]"),
                kind: "person",
                id: Some(p.item.id.clone()),
                fields,
            },
        );
    }
    for s in &model.software_systems {
        index_system_tree(s, canon, index);
    }
    for node in &model.deployment_nodes {
        walk_node(node, canon, index);
    }
    for r in &model.relationships {
        let key = rel_canon(&r.id);
        let tail = bracket(&key);
        insert_item(
            index,
            key,
            Item {
                path: format!("model.relationships[{tail}]"),
                kind: "relationship",
                id: Some(r.id.clone()),
                fields: rel_fields(r),
            },
        );
    }
}

/// One software system with its containers and components — the nesting
/// shares dotted paths, the identity keys are the canonical names.
fn index_system_tree(s: &SoftwareSystem, canon: &dyn Fn(&Id) -> String, index: &mut Index) {
    let s_key = canon(&s.item.id);
    let s_tail = bracket(&s_key);
    let mut fields = base_fields(&s.item, TagKind::SoftwareSystem);
    push(&mut fields, "group", opt(s.group.as_ref()));
    let s_path = format!("model.softwareSystems[{s_tail}]");
    insert_item(
        index,
        s_key,
        Item {
            path: s_path.clone(),
            kind: "softwareSystem",
            id: Some(s.item.id.clone()),
            fields,
        },
    );
    for c in &s.containers {
        let c_key = canon(&c.item.id);
        let mut fields = base_fields(&c.item, TagKind::Container);
        push(&mut fields, "group", opt(c.group.as_ref()));
        push(&mut fields, "technology", opt(c.technology.as_ref()));
        let c_name = &c.item.name;
        let c_path = format!("{s_path}.containers[{c_name}]");
        insert_item(
            index,
            c_key,
            Item {
                path: c_path.clone(),
                kind: "container",
                id: Some(c.item.id.clone()),
                fields,
            },
        );
        for k in &c.components {
            let k_key = canon(&k.item.id);
            let mut fields = base_fields(&k.item, TagKind::Component);
            push(&mut fields, "group", opt(k.group.as_ref()));
            push(&mut fields, "technology", opt(k.technology.as_ref()));
            let k_name = &k.item.name;
            insert_item(
                index,
                k_key,
                Item {
                    path: format!("{c_path}.components[{k_name}]"),
                    kind: "component",
                    id: Some(k.item.id.clone()),
                    fields,
                },
            );
        }
    }
}

/// Recursive deployment-tree walk (children, infra nodes, instances).
fn walk_node(node: &DeploymentNode, canon: &dyn Fn(&Id) -> String, index: &mut Index) {
    let key = canon(&node.item.id);
    let tail = bracket(&key);
    // `DeploymentNode://Default/a/b` → `Default/a/b` under deploymentNodes.
    let path = format!("model.deploymentNodes[{tail}]");
    let mut fields = base_fields(&node.item, TagKind::DeploymentNode);
    push(&mut fields, "group", opt(node.group.as_ref()));
    push(&mut fields, "technology", opt(node.technology.as_ref()));
    push(&mut fields, "instances", json!(&node.instances));
    push(
        &mut fields,
        "deploymentGroups",
        groups(&node.deployment_groups),
    );
    insert_item(
        index,
        key,
        Item {
            path,
            kind: "deploymentNode",
            id: Some(node.item.id.clone()),
            fields,
        },
    );
    for infra in &node.infrastructure_nodes {
        let i_key = canon(&infra.item.id);
        let i_name = bracket(&i_key).rsplit('/').next().unwrap_or("").to_owned();
        let mut fields = base_fields(&infra.item, TagKind::InfrastructureNode);
        push(&mut fields, "group", opt(infra.group.as_ref()));
        push(&mut fields, "technology", opt(infra.technology.as_ref()));
        insert_item(
            index,
            i_key,
            Item {
                path: format!("model.deploymentNodes[{tail}].infrastructureNodes[{i_name}]"),
                kind: "infrastructureNode",
                id: Some(infra.item.id.clone()),
                fields,
            },
        );
    }
    for ssi in &node.software_system_instances {
        let s_key = canon(&ssi.item.id);
        let mut fields = base_fields(&ssi.item, TagKind::SoftwareSystemInstance);
        push(&mut fields, "group", opt(ssi.group.as_ref()));
        push(
            &mut fields,
            "deploymentGroups",
            groups(&ssi.deployment_groups),
        );
        let name = &ssi.item.name;
        let n = ssi.instance_id;
        insert_item(
            index,
            s_key,
            Item {
                path: format!("model.deploymentNodes[{tail}].softwareSystemInstances[{name}#{n}]"),
                kind: "softwareSystemInstance",
                id: Some(ssi.item.id.clone()),
                fields,
            },
        );
    }
    for ci in &node.container_instances {
        let c_key = canon(&ci.item.id);
        let mut fields = base_fields(&ci.item, TagKind::ContainerInstance);
        push(&mut fields, "group", opt(ci.group.as_ref()));
        push(
            &mut fields,
            "deploymentGroups",
            groups(&ci.deployment_groups),
        );
        let name = &ci.item.name;
        let n = ci.instance_id;
        insert_item(
            index,
            c_key,
            Item {
                path: format!("model.deploymentNodes[{tail}].containerInstances[{name}#{n}]"),
                kind: "containerInstance",
                id: Some(ci.item.id.clone()),
                fields,
            },
        );
    }
    for child in &node.children {
        walk_node(child, canon, index);
    }
}

fn rel_fields(r: &Relationship) -> Fields {
    let mut fields = Fields::new();
    push(&mut fields, "technology", opt(r.technology.as_ref()));
    push(&mut fields, "url", opt(r.url.as_ref()));
    let defaults = relationship_defaults(r.linked_relationship_id.is_some(), r.interaction_style);
    push(&mut fields, "tags", tags(&r.tags, &defaults));
    push_props(&mut fields, &r.properties);
    fields
}

fn workspace_fields(ws: &Workspace) -> Fields {
    let mut fields = Fields::new();
    push(&mut fields, "name", opt(ws.name.as_ref()));
    push(&mut fields, "description", opt(ws.description.as_ref()));
    push(&mut fields, "version", opt(ws.version.as_ref()));
    push_props(&mut fields, &ws.properties);
    fields
}

// ---------------------------------------------------------------------------
// Views + styles
// ---------------------------------------------------------------------------

fn index_views(
    ws: &Workspace,
    canon: &dyn Fn(&Id) -> String,
    rel_canon: &dyn Fn(&Id) -> String,
    opts: DiffOptions,
    index: &mut Index,
) {
    for v in ws.views.all_views() {
        let key = v.key().to_owned();
        let mut fields = Fields::new();
        push(&mut fields, "kind", json!(v.kind_label()));
        let base = v.base();
        push(&mut fields, "title", opt(base.title.as_ref()));
        push(&mut fields, "description", opt(base.description.as_ref()));
        push_props(&mut fields, &base.properties);
        push_view_fields(v, canon, rel_canon, opts, &mut fields);
        insert_item(
            index,
            key.clone(),
            Item {
                path: format!("views[{key}]"),
                kind: "view",
                id: None,
                fields,
            },
        );
    }
}

/// The per-kind field set: scope ids, boundary flags, membership
/// and (gated) geometry. One `match` on the borrowed `View` enum.
#[allow(clippy::too_many_lines)] // one arm per `View` variant: the field matrix
fn push_view_fields(
    v: View<'_>,
    canon: &dyn Fn(&Id) -> String,
    rel_canon: &dyn Fn(&Id) -> String,
    opts: DiffOptions,
    fields: &mut Fields,
) {
    match v {
        View::Custom(cv) => {
            push(
                fields,
                "softwareSystemId",
                cv.software_system_id
                    .as_ref()
                    .map_or(Value::Null, |id| json!(canon(id))),
            );
            push_content(fields, &cv.content, canon, rel_canon, opts);
            push_animations(fields, &cv.animations, canon, rel_canon, opts);
        }
        View::Landscape(lv) => {
            push(
                fields,
                "enterpriseBoundaryVisible",
                json!(lv.enterprise_boundary_visible),
            );
            push_content(fields, &lv.content, canon, rel_canon, opts);
            push_animations(fields, &lv.animations, canon, rel_canon, opts);
        }
        View::Context(cv) => {
            push(
                fields,
                "softwareSystemId",
                json!(canon(&cv.software_system_id)),
            );
            push(
                fields,
                "enterpriseBoundaryVisible",
                json!(cv.enterprise_boundary_visible),
            );
            push_content(fields, &cv.content, canon, rel_canon, opts);
            push_animations(fields, &cv.animations, canon, rel_canon, opts);
        }
        View::Container(cv) => {
            push(
                fields,
                "softwareSystemId",
                json!(canon(&cv.software_system_id)),
            );
            push(
                fields,
                "externalSoftwareSystemBoundariesVisible",
                json!(cv.external_software_system_boundaries_visible),
            );
            push_content(fields, &cv.content, canon, rel_canon, opts);
            push_animations(fields, &cv.animations, canon, rel_canon, opts);
        }
        View::Component(cv) => {
            push(fields, "containerId", json!(canon(&cv.container_id)));
            push(
                fields,
                "externalContainerBoundariesVisible",
                json!(cv.external_container_boundaries_visible),
            );
            push_content(fields, &cv.content, canon, rel_canon, opts);
            push_animations(fields, &cv.animations, canon, rel_canon, opts);
        }
        View::Dynamic(dv) => {
            push(
                fields,
                "elementId",
                dv.element_id
                    .as_ref()
                    .map_or(Value::Null, |id| json!(canon(id))),
            );
            push(
                fields,
                "deploymentEnvironment",
                opt(dv.deployment_environment.as_ref()),
            );
            push(
                fields,
                "externalBoundariesVisible",
                json!(dv.external_boundaries_visible),
            );
            push_content(fields, &dv.content, canon, rel_canon, opts);
        }
        View::Deployment(dv) => {
            push(fields, "environment", json!(&dv.environment));
            push(
                fields,
                "softwareSystemId",
                dv.software_system_id
                    .as_ref()
                    .map_or(Value::Null, |id| json!(canon(id))),
            );
            push_content(fields, &dv.content, canon, rel_canon, opts);
            push_animations(fields, &dv.animations, canon, rel_canon, opts);
        }
        View::Filtered(fv) => {
            push(fields, "baseViewKey", json!(&fv.base_view_key));
            push(
                fields,
                "filterMode",
                fv.mode.map_or(Value::Null, |m| json!(m.as_str())),
            );
            push(fields, "filterTags", json!(&fv.tags));
        }
        View::Image(_) => {
            // Content is base64 payload — never compared (an earlier milestone).
        }
    }
}

/// Membership (always) + geometry (gated by `ignore_layout`).
fn push_content(
    fields: &mut Fields,
    content: &ViewContent,
    canon: &dyn Fn(&Id) -> String,
    rel_canon: &dyn Fn(&Id) -> String,
    opts: DiffOptions,
) {
    let mut elements: Vec<String> = content.elements.iter().map(|e| canon(&e.id)).collect();
    elements.sort();
    push(fields, "elements", json!(elements));
    let mut relationships: Vec<String> = content
        .relationships
        .iter()
        .map(|r| rel_canon(&r.id))
        .collect();
    relationships.sort();
    push(fields, "relationships", json!(relationships));
    push(
        fields,
        "paperSize",
        content
            .paper_size
            .map_or(Value::Null, |p| json!(p.as_str())),
    );
    if opts.ignore_layout {
        return;
    }
    for e in &content.elements {
        push(
            fields,
            &format!("position[{}]", bracket(&canon(&e.id))),
            json!({ "x": e.x, "y": e.y }),
        );
    }
    for r in &content.relationships {
        if !r.vertices.is_empty() {
            let vertices: Vec<Value> = r
                .vertices
                .iter()
                .map(|v| json!({ "x": v.x, "y": v.y }))
                .collect();
            push(
                fields,
                &format!("vertices[{}]", bracket(&rel_canon(&r.id))),
                json!(vertices),
            );
        }
    }
    if let Some(al) = &content.automatic_layout {
        push(fields, "automaticLayout", auto_layout_value(al));
    }
    if let Some(d) = &content.dimensions {
        push(fields, "dimensions", dimensions_value(*d));
    }
}

fn push_animations(
    fields: &mut Fields,
    animations: &[Animation],
    canon: &dyn Fn(&Id) -> String,
    rel_canon: &dyn Fn(&Id) -> String,
    opts: DiffOptions,
) {
    if opts.ignore_layout || animations.is_empty() {
        return;
    }
    let steps: Vec<Value> = animations
        .iter()
        .map(|a| {
            let elements: Vec<String> = a
                .element_ids
                .iter()
                .map(|id| canon(&Id::new(id.clone())))
                .map(|c| bracket(&c))
                .collect();
            let relationships: Vec<String> = a
                .relationship_ids
                .iter()
                .map(|id| bracket(&rel_canon(&Id::new(id.clone()))))
                .collect();
            json!({
                "order": a.order,
                "elements": elements,
                "relationships": relationships,
            })
        })
        .collect();
    push(fields, "animations", json!(steps));
}

fn auto_layout_value(al: &AutomaticLayout) -> Value {
    json!({
        "rankDirection": al.rank_direction.map_or(Value::Null, |d| json!(d.as_str())),
        "rankSeparation": al.rank_separation,
        "nodeSeparation": al.node_separation,
        "edgeSeparation": al.edge_separation,
        "vertices": al.vertices,
    })
}

fn dimensions_value(d: Dimensions) -> Value {
    json!({ "width": d.width, "height": d.height })
}

fn index_styles(ws: &Workspace, index: &mut Index) {
    let styles = &ws.views.configuration.styles;
    for s in &styles.element_styles {
        let tag = &s.tag;
        insert_item(
            index,
            format!("element\0{tag}"),
            Item {
                path: format!("views.styles.elements[{tag}]"),
                kind: "elementStyle",
                id: None,
                fields: element_style_fields(s),
            },
        );
    }
    for s in &styles.relationship_styles {
        let tag = &s.tag;
        insert_item(
            index,
            format!("relationship\0{tag}"),
            Item {
                path: format!("views.styles.relationships[{tag}]"),
                kind: "relationshipStyle",
                id: None,
                fields: relationship_style_fields(s),
            },
        );
    }
}

/// Every paint field of an `ElementStyle` (: styles by (kind, tag)).
fn element_style_fields(s: &ElementStyle) -> Fields {
    let mut fields = Fields::new();
    push(
        &mut fields,
        "colorScheme",
        s.color_scheme.map_or(Value::Null, |e| json!(e.as_str())),
    );
    push(
        &mut fields,
        "width",
        s.width.map_or(Value::Null, |v| json!(v)),
    );
    push(
        &mut fields,
        "height",
        s.height.map_or(Value::Null, |v| json!(v)),
    );
    push(&mut fields, "background", opt(s.background.as_ref()));
    push(&mut fields, "stroke", opt(s.stroke.as_ref()));
    push(
        &mut fields,
        "strokeWidth",
        s.stroke_width.map_or(Value::Null, |v| json!(v)),
    );
    push(&mut fields, "color", opt(s.color.as_ref()));
    push(
        &mut fields,
        "fontSize",
        s.font_size.map_or(Value::Null, |v| json!(v)),
    );
    push(
        &mut fields,
        "shape",
        s.shape.map_or(Value::Null, |e| json!(e.as_str())),
    );
    push(&mut fields, "icon", opt(s.icon.as_ref()));
    push(
        &mut fields,
        "iconPosition",
        s.icon_position.map_or(Value::Null, |e| json!(e.as_str())),
    );
    push(
        &mut fields,
        "border",
        s.border.map_or(Value::Null, |e| json!(e.as_str())),
    );
    push(
        &mut fields,
        "opacity",
        s.opacity.map_or(Value::Null, |v| json!(v)),
    );
    push(
        &mut fields,
        "metadata",
        s.metadata.map_or(Value::Null, |v| json!(v)),
    );
    push(
        &mut fields,
        "description",
        s.description.map_or(Value::Null, |v| json!(v)),
    );
    push_props(&mut fields, &s.properties);
    fields
}

/// Every paint field of a `RelationshipStyle`.
fn relationship_style_fields(s: &RelationshipStyle) -> Fields {
    let mut fields = Fields::new();
    push(
        &mut fields,
        "colorScheme",
        s.color_scheme.map_or(Value::Null, |e| json!(e.as_str())),
    );
    push(
        &mut fields,
        "thickness",
        s.thickness.map_or(Value::Null, |v| json!(v)),
    );
    push(&mut fields, "color", opt(s.color.as_ref()));
    push(
        &mut fields,
        "fontSize",
        s.font_size.map_or(Value::Null, |v| json!(v)),
    );
    push(
        &mut fields,
        "width",
        s.width.map_or(Value::Null, |v| json!(v)),
    );
    push(
        &mut fields,
        "dashed",
        s.dashed.map_or(Value::Null, |v| json!(v)),
    );
    push(
        &mut fields,
        "style",
        s.style.map_or(Value::Null, |e| json!(e.as_str())),
    );
    push(
        &mut fields,
        "routing",
        s.routing.map_or(Value::Null, |e| json!(e.as_str())),
    );
    push(
        &mut fields,
        "jump",
        s.jump.map_or(Value::Null, |v| json!(v)),
    );
    push(
        &mut fields,
        "position",
        s.position.map_or(Value::Null, |v| json!(v)),
    );
    push(
        &mut fields,
        "opacity",
        s.opacity.map_or(Value::Null, |v| json!(v)),
    );
    push(
        &mut fields,
        "metadata",
        s.metadata.map_or(Value::Null, |v| json!(v)),
    );
    push(
        &mut fields,
        "description",
        s.description.map_or(Value::Null, |v| json!(v)),
    );
    push_props(&mut fields, &s.properties);
    fields
}

// ---------------------------------------------------------------------------
// Field plumbing
// ---------------------------------------------------------------------------

/// The `ModelItem` fields every element shares. `kind` selects the
/// derived-default tag stack the EFFECTIVE view is computed against (the
/// loaders represent the same list differently).
fn base_fields(item: &ModelItem, kind: TagKind) -> Fields {
    let mut fields = Fields::new();
    push(&mut fields, "name", json!(&item.name));
    push(&mut fields, "description", opt(item.description.as_ref()));
    push(&mut fields, "url", opt(item.url.as_ref()));
    push(&mut fields, "tags", tags(&item.tags, kind.default_tags()));
    push_props(&mut fields, &item.properties);
    fields
}

/// Insert a field (the `Value::Null` = absent convention is uniform, so
/// pushing defaults is harmless).
fn push(fields: &mut Fields, name: &str, value: Value) {
    fields.insert(name.to_owned(), value);
}

/// An empty string is `""` on one loader and absent on the other (the
/// the official tooling `ModelItem` defaults `description` to `""`, the JSON writer
/// omits empty text), so both read as ABSENT — the same
/// format-agnostic rule as the effective tags view).
fn opt(o: Option<&String>) -> Value {
    match o {
        Some(s) if s.is_empty() => Value::Null,
        Some(s) => json!(s),
        None => Value::Null,
    }
}

/// The EFFECTIVE tag list (`getTags` = defaults ++ stored, deduped):
/// the DSL path keeps kind defaults DERIVED while a JSON load re-stores
/// the CSV, so stored-only comparison would flag every cross-format pair.
fn tags(t: &oxidizr_core::tags::Tags, defaults: &[&str]) -> Value {
    json!(t.effective(defaults))
}

fn groups(g: &[String]) -> Value {
    json!(g)
}

/// One `properties.<key>` field per entry (per-key granularity, an earlier milestone).
fn push_props(fields: &mut Fields, props: &Properties) {
    for (k, v) in props.iter() {
        // Provenance stamps are NEVER compared: the DSL loader regenerates
        // them on every parse (`structurizr.dsl.identifier` = a fresh UUID
        // per item, `structurizr.dsl` = the base64 source snapshot), so
        // including them would break the `diff(a, a)` EMPTY property on
        // every parsed input (documented in diff-schema.md).
        if k.starts_with("structurizr.dsl") {
            continue;
        }
        push(fields, &format!("properties.{k}"), json!(v));
    }
}

/// Emit one [`Change`] per field whose value differs (missing = `Null`).
fn compare_fields(path: &str, a: &Fields, b: &Fields, out: &mut Vec<Change>) {
    let keys: std::collections::BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    for key in keys {
        let before = a.get(key).cloned().unwrap_or(Value::Null);
        let after = b.get(key).cloned().unwrap_or(Value::Null);
        if before != after {
            out.push(Change {
                path: path.to_owned(),
                field: key.clone(),
                before,
                after,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::expect;
    use oxidizr_core::model::{Person, SoftwareSystem};
    use oxidizr_core::tags::Tags;
    use oxidizr_core::views::{
        ElementStyle, ElementView, SystemLandscapeView, ViewBase, ViewContent,
    };

    fn ws() -> Workspace {
        let mut ws = Workspace {
            name: Some("W".into()),
            ..Workspace::default()
        };
        ws.model.people.push(Person {
            item: ModelItem::new(Id::from_num(1), "Alice"),
            group: Some("Ops".into()),
            relationships: vec![],
        });
        ws.model.software_systems.push(SoftwareSystem {
            item: ModelItem::new(Id::from_num(2), "Pay"),
            group: None,
            containers: vec![],
            documentation: oxidizr_core::docs::Documentation::default(),
            relationships: vec![],
        });
        ws.model.relationships.push(Relationship {
            id: Id::from_num(3),
            source: Id::from_num(1),
            destination: Id::from_num(2),
            description: Some("uses".into()),
            technology: Some("HTTPS".into()),
            tags: Tags::new(),
            url: None,
            properties: Properties::new(),
            perspectives: vec![],
            interaction_style: None,
            linked_relationship_id: None,
        });
        ws.views.system_landscape_views.push(SystemLandscapeView {
            base: ViewBase::new("L1", 1),
            content: ViewContent {
                elements: vec![ElementView {
                    id: Id::from_num(1),
                    x: 10,
                    y: 20,
                }],
                relationships: vec![],
                paper_size: None,
                dimensions: None,
                automatic_layout: None,
            },
            enterprise_boundary_visible: true,
            animations: vec![],
        });
        let style = ElementStyle {
            tag: "Person".into(),
            background: Some("#ff0000".into()),
            ..ElementStyle::default()
        };
        ws.views.configuration.styles.element_styles.push(style);
        ws
    }

    #[test]
    fn identical_workspaces_diff_empty() {
        let a = ws();
        let d = diff(&a, &a, &DiffOptions::default());
        assert_eq!(d, Diff::default(), "diff(a,a) must be empty: {d:?}");
    }

    #[test]
    fn rename_is_add_remove_and_carries_the_relationship() {
        let a = ws();
        let mut b = ws();
        b.model.people[0].item.name = "Alicia".into();
        b.model.people.push(Person {
            item: ModelItem::new(Id::from_num(9), "Bob"),
            group: None,
            relationships: vec![],
        });
        let d = diff(&a, &b, &DiffOptions::default());
        // The rename is an IDENTITY change: remove + add, never `changed`.
        // Locked-in consequence: the relationship key EMBEDS the
        // endpoint names, so Alice's rename moves the relationship identity
        // too — remove + add pair, and the view fields follow the new name.
        assert_eq!(
            d.removed
                .iter()
                .map(|e| e.path.as_str())
                .collect::<Vec<_>>(),
            [
                "model.people[Alice]",
                "model.relationships[Person://Alice -> SoftwareSystem://Pay (uses)]",
            ]
        );
        assert_eq!(
            d.added.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(),
            [
                "model.people[Alicia]",
                "model.people[Bob]",
                "model.relationships[Person://Alicia -> SoftwareSystem://Pay (uses)]",
            ]
        );
        assert!(
            d.changed.iter().any(|c| c.path == "views[L1]"
                && c.field == "elements"
                && c.before == json!(["Person://Alice"])
                && c.after == json!(["Person://Alicia"])),
            "{:?}",
            d.changed
        );
        assert!(
            d.changed.iter().all(|c| c.path == "views[L1]"),
            "{:?}",
            d.changed
        );
    }

    #[test]
    fn field_changes_report_before_and_after() {
        let a = ws();
        let mut b = ws();
        b.model.people[0].item.description = Some("CEO".into());
        b.model.people[0].item.properties.insert("role", "lead");
        b.model.software_systems[0].item.url = Some("https://pay".into());
        b.model.relationships[0].technology = Some("gRPC".into());
        let d = diff(&a, &b, &DiffOptions::default());
        let find = |path: &str, field: &str| {
            d.changed
                .iter()
                .find(|c| c.path == path && c.field == field)
                .cloned()
        };
        let desc = find("model.people[Alice]", "description").expect("description");
        assert_eq!(desc.before, Value::Null);
        assert_eq!(desc.after, json!("CEO"));
        let prop = find("model.people[Alice]", "properties.role").expect("property");
        assert_eq!(prop.before, Value::Null);
        assert_eq!(prop.after, json!("lead"));
        assert!(find("model.softwareSystems[Pay]", "url").is_some());
        let rel_tail = bracket(&canonical_rel(&b));
        let tech = find(&format!("model.relationships[{rel_tail}]"), "technology")
            .unwrap_or_else(|| panic!("expected a technology change: {:?}", d.changed));
        assert_eq!(tech.before, json!("HTTPS"));
        assert_eq!(tech.after, json!("gRPC"));
        assert!(d.added.is_empty() && d.removed.is_empty(), "{d:?}");
    }

    fn canonical_rel(ws: &Workspace) -> String {
        let mut b = WorkspaceBuilder::new();
        b.adopt(ws.clone());
        b.relationship_canonical_name(&Id::from_num(3))
            .unwrap_or_default()
    }

    #[test]
    fn consistent_id_shift_is_id_changed_never_add_remove() {
        let a = ws();
        let mut b = ws();
        // A REAL id shift re-points every the official tooling (the id-shift scenario):
        // a half-shifted workspace is dangling, not an id drift.
        b.model.people[0].item.id = Id::from_num(123);
        b.model.relationships[0].source = Id::from_num(123);
        b.model.relationships[0].id = Id::from_num(42);
        b.views.system_landscape_views[0].content.elements[0].id = Id::from_num(123);
        let d = diff(&a, &b, &DiffOptions::default());
        assert!(d.added.is_empty() && d.removed.is_empty(), "{d:?}");
        assert!(d.changed.is_empty(), "{:?}", d.changed);
        assert_eq!(d.id_changed.len(), 2, "{:?}", d.id_changed);
        assert_eq!(d.id_changed[0].path, "model.people[Alice]");
        assert_eq!(d.id_changed[0].before, "1");
        assert_eq!(d.id_changed[0].after, "123");
        assert_eq!(d.id_changed[1].before, "3");
        assert_eq!(d.id_changed[1].after, "42");
    }

    #[test]
    fn relationship_description_change_is_add_remove() {
        let a = ws();
        let mut b = ws();
        b.model.relationships[0].description = Some("calls".into());
        let d = diff(&a, &b, &DiffOptions::default());
        assert_eq!(d.added.len(), 1, "new identity … {d:?}");
        assert_eq!(d.removed.len(), 1);
        assert_eq!(d.changed, Vec::new());
    }

    #[test]
    fn view_geometry_gates_on_ignore_layout() {
        let a = ws();
        let mut b = ws();
        b.views.system_landscape_views[0].content.elements[0].x = 11;
        let d = diff(&a, &b, &DiffOptions::default());
        let pos = d
            .changed
            .iter()
            .find(|c| c.field == "position[Alice]")
            .cloned()
            .expect("position change");
        assert_eq!(pos.before, json!({ "x": 10, "y": 20 }));
        assert_eq!(pos.after, json!({ "x": 11, "y": 20 }));
        let d2 = diff(
            &a,
            &b,
            &DiffOptions {
                ignore_layout: true,
            },
        );
        assert!(d2.is_empty(), "layout ignored must be empty: {d2:?}");
    }

    #[test]
    fn view_membership_ignores_the_layout_flag() {
        let a = ws();
        let mut b = ws();
        b.views.system_landscape_views[0]
            .content
            .elements
            .push(ElementView {
                id: Id::from_num(2),
                x: 0,
                y: 0,
            });
        for opts in [
            DiffOptions::default(),
            DiffOptions {
                ignore_layout: true,
            },
        ] {
            let d = diff(&a, &b, &opts);
            let membership = d
                .changed
                .iter()
                .find(|c| c.field == "elements")
                .unwrap_or_else(|| panic!("{opts:?}: {d:?}"));
            assert_eq!(
                membership.after.as_array().unwrap().len(),
                2,
                "membership is not layout"
            );
        }
    }

    #[test]
    fn styles_compare_by_tag_kind() {
        let a = ws();
        let mut b = ws();
        b.views.configuration.styles.element_styles[0].background = Some("#00ff00".into());
        let extra = oxidizr_core::views::RelationshipStyle {
            tag: "Person".into(), // relationship kind: no collision
            ..oxidizr_core::views::RelationshipStyle::default()
        };
        b.views.configuration.styles.relationship_styles.push(extra);
        let d = diff(&a, &b, &DiffOptions::default());
        let bg = d
            .changed
            .iter()
            .find(|c| c.path == "views.styles.elements[Person]" && c.field == "background")
            .expect("background");
        assert_eq!(bg.after, json!("#00ff00"));
        assert_eq!(
            d.added
                .iter()
                .map(|e| (e.path.as_str(), e.kind))
                .collect::<Vec<_>>(),
            [("views.styles.relationships[Person]", "relationshipStyle")]
        );
    }

    #[test]
    fn workspace_metadata_changes() {
        let a = ws();
        let mut b = ws();
        b.name = Some("W2".into());
        b.version = Some("v2".into());
        let d = diff(&a, &b, &DiffOptions::default());
        assert_eq!(d.changed.len(), 2);
        assert!(d.changed.iter().all(|c| c.path == "workspace"));
    }

    #[test]
    fn cross_loader_representation_diffs_empty() {
        // The same model on both loaders: the DSL path keeps kind defaults
        // DERIVED (stored tags empty, `description` defaults to ""), a JSON
        // load re-stores the tag CSV and omits empty text. Comparison goes
        // through the EFFECTIVE views, so the pair must diff EMPTY
        // (caught by the `testdata/probes/diff` cross pair).
        let mut a = ws();
        a.model.people[0].item.description = Some(String::new());
        let mut b = ws();
        b.model.people[0].item.description = None;
        b.model.people[0].item.tags = Tags::from_json_csv("Element,Person");
        b.model.relationships[0].tags = Tags::from_json_csv("Relationship");
        b.model.software_systems[0].item.tags.add("Extra"); // custom tag present on both sides
        a.model.software_systems[0].item.tags.add("Extra");
        assert!(
            diff(&a, &b, &DiffOptions::default()).is_empty(),
            "{:?}",
            diff(&a, &b, &DiffOptions::default())
        );
    }

    #[test]
    fn provenance_stamps_are_never_compared() {
        // The DSL loader stamps `structurizr.dsl.identifier` (fresh UUID)
        // and the `structurizr.dsl` source snapshot on every parse — the
        // `diff(a, a)` EMPTY property only holds when both are excluded.
        let a = ws();
        let mut b = ws();
        b.model.people[0].item.properties.insert(
            "structurizr.dsl.identifier",
            "9a9f97b3-10de-4fc5-b737-6dda095a1f3b",
        );
        b.properties.insert("structurizr.dsl", "ZHlmZg==");
        let mut with = ws();
        with.model.people[0].item.properties.insert(
            "structurizr.dsl.identifier",
            "11111111-2222-3333-4444-555555555555",
        );
        with.properties.insert("structurizr.dsl", "d29ya3NwYWNl");
        assert!(
            diff(&with, &a, &DiffOptions::default()).is_empty(),
            "stamps on both sides"
        );
        let d = diff(&a, &b, &DiffOptions::default());
        assert!(d.is_empty(), "differing stamps must be invisible: {d:?}");
    }

    #[test]
    fn duplicate_identity_pairs_positionally() {
        let mut a = ws();
        a.model.people.push(Person {
            item: ModelItem::new(Id::from_num(7), "Alice"),
            group: None,
            relationships: vec![],
        });
        let mut b = a.clone();
        b.model.people[1].item.description = Some("twin".into());
        let d = diff(&a, &b, &DiffOptions::default());
        // Second occurrence pairs with second occurrence.
        assert!(d.added.is_empty() && d.removed.is_empty(), "{d:?}");
        assert_eq!(d.changed.len(), 1);
        assert_eq!(d.changed[0].path, "model.people[Alice]");
        assert_eq!(d.changed[0].after, json!("twin"));
    }

    #[test]
    fn schema_one_document_snapshot() {
        // One of every category: rename (add+remove pair incl. the
        // relationship), a field change, a property change and a
        // consistent id shift. The pretty serialization is the CLI
        // `--format json` payload (M0c) — key order and shape are the
        // frozen schema (docs/diff-schema.md).
        let a = ws();
        let mut b = ws();
        b.model.people[0].item.name = "Alicia".into();
        b.model.software_systems[0].item.url = Some("https://pay".into());
        b.model.software_systems[0]
            .item
            .properties
            .insert("owner", "team-a");
        b.model.relationships[0].destination = Id::from_num(77);
        b.model.software_systems[0].item.id = Id::from_num(77);
        let d = diff(&a, &b, &DiffOptions::default());
        expect![[r#"
            {
              "schema": 1,
              "added": [
                {
                  "path": "model.people[Alicia]",
                  "kind": "person"
                },
                {
                  "path": "model.relationships[Person://Alicia -> SoftwareSystem://Pay (uses)]",
                  "kind": "relationship"
                }
              ],
              "removed": [
                {
                  "path": "model.people[Alice]",
                  "kind": "person"
                },
                {
                  "path": "model.relationships[Person://Alice -> SoftwareSystem://Pay (uses)]",
                  "kind": "relationship"
                }
              ],
              "changed": [
                {
                  "path": "model.softwareSystems[Pay]",
                  "field": "properties.owner",
                  "before": null,
                  "after": "team-a"
                },
                {
                  "path": "model.softwareSystems[Pay]",
                  "field": "url",
                  "before": null,
                  "after": "https://pay"
                },
                {
                  "path": "views[L1]",
                  "field": "elements",
                  "before": [
                    "Person://Alice"
                  ],
                  "after": [
                    "Person://Alicia"
                  ]
                },
                {
                  "path": "views[L1]",
                  "field": "position[Alice]",
                  "before": {
                    "x": 10,
                    "y": 20
                  },
                  "after": null
                },
                {
                  "path": "views[L1]",
                  "field": "position[Alicia]",
                  "before": null,
                  "after": {
                    "x": 10,
                    "y": 20
                  }
                }
              ],
              "idChanged": [
                {
                  "path": "model.softwareSystems[Pay]",
                  "before": "2",
                  "after": "77"
                }
              ]
            }"#]]
        .assert_eq(&serde_json::to_string_pretty(&d.to_json()).unwrap());
    }
}
