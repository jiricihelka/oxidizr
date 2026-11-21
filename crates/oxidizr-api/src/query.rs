//! `oxidizr query` report layer (§P4-M3 O-1, F6).
//!
//! The model-side evaluator is the pinned `ExpressionParser` port —
//! reached through the public [`oxidizr_dsl::query`] entry (one parser,
//! one evaluator: filtered views and `query` are its two consumers).
//! Views have NO reference expression grammar; for them a documented
//! SUBSET of key/title/description comparisons matches — our own F6
//! surface, cataloged in `docs/query-schema.md`.
//!
//! This module is the pure REPORT layer: rows and bytes only. The
//! command body lives in [`crate::pipeline`] (the T0-d invariant, like
//! `inspect`); the command is pure READ — the workspace never mutates,
//! no diagnostic codes are invented, and the JSON output is a
//! schema-versioned report (P4-D7: no `oxidizr-json` byte contact).

use oxidizr_core::WorkspaceBuilder;
use oxidizr_core::configuration::Workspace;
use oxidizr_core::tags::{TagKind, relationship_defaults};
use oxidizr_core::validate::ElementKind;
use oxidizr_core::views::View;
use serde_json::{Value, json};

/// Which report classes `--kind` includes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryKind {
    /// Matching elements only.
    Element,
    /// Matching relationships only.
    Relationship,
    /// Matching views only (the documented view-subset grammar).
    View,
    /// Elements, relationships AND views.
    All,
}

/// Output shape (`--format`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryFormat {
    /// Human rows `id  kind  name  [tags]` (the default).
    Table,
    /// One id (or view key) per line — pipeline-friendly.
    Ids,
    /// Schema-1 document (`docs/query-schema.md`).
    Json,
}

/// `--kind` token (the schema `kind` field and the CLI surface share it).
#[must_use]
pub const fn kind_name(kind: QueryKind) -> &'static str {
    match kind {
        QueryKind::Element => "element",
        QueryKind::Relationship => "relationship",
        QueryKind::View => "view",
        QueryKind::All => "all",
    }
}

/// Render the full report for one query. `model` is the
/// [`oxidizr_dsl::query`] result for `filter` (ignored when `kind` is
/// [`QueryKind::View`]). The only failure mode is the JSON serializer.
///
/// # Errors
///
/// Returns the serializer message when the schema document cannot be
/// rendered (unreachable for workspace-derived values).
pub(crate) fn report_bytes(
    ws: &Workspace,
    filter: &str,
    kind: QueryKind,
    format: QueryFormat,
    model: &oxidizr_dsl::query::QueryResult,
) -> Result<Vec<u8>, String> {
    let rows = report_rows(ws, filter, kind, model);
    Ok(match format {
        QueryFormat::Table => table_bytes(&rows),
        QueryFormat::Ids => ids_bytes(&rows),
        QueryFormat::Json => json_bytes(filter, kind, &rows)?,
    })
}

// ---------------------------------------------------------------------------
// rows
// ---------------------------------------------------------------------------

/// One reported row (uniform across classes so every format stays simple).
struct Row {
    id: String,
    class: RowClass,
    kind: String,
    name: Option<String>,
    tags: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RowClass {
    Element,
    Relationship,
    View,
}

/// Build the reported rows in report order: elements (engine result
/// order), then relationships (engine result order), then views
/// (workspace view order). Canonical names come from an adopted
/// [`WorkspaceBuilder`] — the `diff` identity machinery (D-4): the same
/// view a `.dsl` and its JSON twin agree on.
fn report_rows(
    ws: &Workspace,
    filter: &str,
    kind: QueryKind,
    model: &oxidizr_dsl::query::QueryResult,
) -> Vec<Row> {
    let mut b = WorkspaceBuilder::new();
    b.adopt(ws.clone());
    let mut rows = Vec::new();
    if matches!(kind, QueryKind::Element | QueryKind::All) {
        for id in &model.elements {
            let Some(item) = b.item(id) else { continue };
            let element_kind = b.element_kind(id);
            let tags = element_kind.map_or_else(Vec::new, |k| {
                effective(
                    item.tags
                        .effective(TagKind::from_element_kind(k).default_tags()),
                )
            });
            rows.push(Row {
                id: id.as_str().to_string(),
                class: RowClass::Element,
                kind: element_kind
                    .map_or("Element", element_kind_label)
                    .to_string(),
                name: b.canonical_name_full(id),
                tags,
            });
        }
    }
    if matches!(kind, QueryKind::Relationship | QueryKind::All) {
        for id in &model.relationships {
            let rel = ws.model.relationships.iter().find(|r| r.id == *id);
            let tags = rel.map_or_else(Vec::new, relationship_tags);
            rows.push(Row {
                id: id.as_str().to_string(),
                class: RowClass::Relationship,
                kind: "Relationship".to_string(),
                name: b.relationship_canonical_name(id),
                tags,
            });
        }
    }
    if matches!(kind, QueryKind::View | QueryKind::All) {
        for view in ws.views.all_views() {
            let base = view.base();
            if view_matches(filter, base) {
                rows.push(Row {
                    id: base.key.clone(),
                    class: RowClass::View,
                    kind: view_kind_token(&view).to_string(),
                    name: base.title.clone(),
                    tags: Vec::new(),
                });
            }
        }
    }
    rows
}

/// The EFFECTIVE tag list (`getTags()` view — the loader-agnostic rule
/// first pinned by `diff`, §P4-M0 D-4).
fn effective(tags: Vec<&str>) -> Vec<String> {
    tags.into_iter().map(str::to_string).collect()
}

/// Relationship EFFECTIVE tags: the derived `Relationship` (+ style)
/// defaults plus the stored list.
fn relationship_tags(rel: &oxidizr_core::model::Relationship) -> Vec<String> {
    let defaults =
        relationship_defaults(rel.linked_relationship_id.is_some(), rel.interaction_style);
    effective(rel.tags.effective(&defaults))
}

/// The view `kind` token — the JSON section names (`customViews` →
/// `Custom`, …), stable across formats (NOT the lowercase
/// [`View::kind_label`] prose).
fn view_kind_token(view: &View<'_>) -> &'static str {
    match view {
        View::Custom(_) => "Custom",
        View::Landscape(_) => "SystemLandscape",
        View::Context(_) => "SystemContext",
        View::Container(_) => "Container",
        View::Component(_) => "Component",
        View::Dynamic(_) => "Dynamic",
        View::Deployment(_) => "Deployment",
        View::Filtered(_) => "Filtered",
        View::Image(_) => "Image",
    }
}

/// `ElementKind` label — the enum spellings the grammar type token uses.
fn element_kind_label(kind: ElementKind) -> &'static str {
    match kind {
        ElementKind::CustomElement => "CustomElement",
        ElementKind::Person => "Person",
        ElementKind::SoftwareSystem => "SoftwareSystem",
        ElementKind::Container => "Container",
        ElementKind::Component => "Component",
        ElementKind::DeploymentNode => "DeploymentNode",
        ElementKind::InfrastructureNode => "InfrastructureNode",
        ElementKind::SoftwareSystemInstance => "SoftwareSystemInstance",
        ElementKind::ContainerInstance => "ContainerInstance",
    }
}

// ---------------------------------------------------------------------------
// the view-subset grammar (our F6 surface — documented per arm)
// ---------------------------------------------------------------------------

/// Match one view against `filter` with the view-subset grammar:
/// `key|title|description ==|!= <value>` (case-insensitive arm prefix —
/// like the reference grammar — but CASE-SENSITIVE value comparison,
/// Java `equals`). The combinators mirror the reference
/// `ExpressionParser.parseExpression` EXACTLY: `" && "` is checked
/// FIRST (a later `||` never wins), the split halves go straight to the
/// single-arm evaluator (a half still containing `" || "` matches no
/// arm — `evaluateExpression` never recurses), and extra arms are
/// dropped like Java's `split` + `[0]`/`[1]`. An arm outside the subset
/// matches NOTHING — the fail-soft the base parser's `default:` arm
/// applies to unmatched model expressions.
fn view_matches(filter: &str, base: &oxidizr_core::views::ViewBase) -> bool {
    if let Some((a, b)) = split_two_arms(filter, " && ") {
        return view_arm(a, base) && view_arm(b, base);
    }
    if let Some((a, b)) = split_two_arms(filter, " || ") {
        return view_arm(a, base) || view_arm(b, base);
    }
    view_arm(filter, base)
}

/// The single-arm evaluator (the `evaluateExpression` analogue): the
/// `!=` arms are checked before `==` in the reference chain order, and
/// the WHOLE remainder after the arm prefix is the value — so a half
/// like `title==A || key==B` compares the literal `"A || key==B"`.
fn view_arm(expr: &str, base: &oxidizr_core::views::ViewBase) -> bool {
    let (arm, value, negate) = if let Some(rest) = strip_arm(expr, "key!=") {
        ("key", rest, true)
    } else if let Some(rest) = strip_arm(expr, "title!=") {
        ("title", rest, true)
    } else if let Some(rest) = strip_arm(expr, "description!=") {
        ("description", rest, true)
    } else if let Some(rest) = strip_arm(expr, "key==") {
        ("key", rest, false)
    } else if let Some(rest) = strip_arm(expr, "title==") {
        ("title", rest, false)
    } else if let Some(rest) = strip_arm(expr, "description==") {
        ("description", rest, false)
    } else {
        return false;
    };
    let lhs = match arm {
        "key" => Some(base.key.as_str()),
        "title" => base.title.as_deref(),
        _ => base.description.as_deref(),
    };
    // Java `equals` against the raw value; an absent title/description
    // only compares equal to nothing (`equals(null)` is false).
    let hit = lhs.is_some_and(|l| l == value);
    hit != negate
}

/// The reference split + `[0]`/`[1]` (`contains`-guarded, so both halves
/// always exist here; Java's trailing-empty `[]`/OOB case keeps its
/// message on the MODEL side via `java_index_or_oob` — on this
/// documented subset the degenerate halves just match no arm).
fn split_two_arms<'a>(expr: &'a str, sep: &str) -> Option<(&'a str, &'a str)> {
    if !expr.contains(sep) {
        return None;
    }
    let mut parts = expr.split(sep);
    let first = parts.next()?;
    let second = parts.next()?;
    Some((first, second))
}

/// Case-insensitive arm prefix; the value keeps its RAW case.
fn strip_arm<'a>(expr: &'a str, arm: &str) -> Option<&'a str> {
    expr.to_ascii_lowercase()
        .starts_with(arm)
        .then(|| &expr[arm.len()..])
}

// ---------------------------------------------------------------------------
// formats
// ---------------------------------------------------------------------------

fn table_bytes(rows: &[Row]) -> Vec<u8> {
    let mut text = String::new();
    for row in rows {
        let tags = row.tags.join(",");
        let mut line = vec![
            row.id.as_str(),
            row.kind.as_str(),
            row.name.as_deref().unwrap_or(""),
        ];
        if !row.tags.is_empty() {
            line.push(&tags);
        }
        text.push_str(&line.join("  "));
        text.push('\n');
    }
    text.into_bytes()
}

fn ids_bytes(rows: &[Row]) -> Vec<u8> {
    let mut text = String::new();
    for row in rows {
        text.push_str(&row.id);
        text.push('\n');
    }
    text.into_bytes()
}

fn json_bytes(filter: &str, kind: QueryKind, rows: &[Row]) -> Result<Vec<u8>, String> {
    let entries = |class: RowClass| -> Vec<Value> {
        rows.iter()
            .filter(|r| r.class == class)
            .map(|r| match class {
                RowClass::View => json!({
                    "key": r.id,
                    "kind": r.kind,
                    "title": r.name,
                }),
                _ => json!({
                    "id": r.id,
                    "kind": r.kind,
                    "canonicalName": r.name,
                    "tags": r.tags,
                }),
            })
            .collect()
    };
    let mut doc = json!({ "schema": 1, "filter": filter, "kind": kind_name(kind) });
    let obj = doc
        .as_object_mut()
        .ok_or_else(|| "report document is not an object".to_string())?;
    if matches!(kind, QueryKind::Element | QueryKind::All) {
        obj.insert("elements".into(), json!(entries(RowClass::Element)));
    }
    if matches!(kind, QueryKind::Relationship | QueryKind::All) {
        obj.insert(
            "relationships".into(),
            json!(entries(RowClass::Relationship)),
        );
    }
    if matches!(kind, QueryKind::View | QueryKind::All) {
        obj.insert("views".into(), json!(entries(RowClass::View)));
    }
    let doc = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    Ok(format!("{doc}\n").into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidizr_core::views::ViewBase;

    fn base() -> ViewBase {
        ViewBase {
            key: "SystemLandscape-001".into(),
            order: 0,
            title: Some("Landscape".into()),
            description: Some("The big picture".into()),
            generated_key: true,
            properties: oxidizr_core::properties::Properties::default(),
        }
    }

    #[test]
    fn view_subset_equality_arms() {
        let b = base();
        assert!(view_matches("key==SystemLandscape-001", &b));
        assert!(view_matches("title==Landscape", &b));
        assert!(view_matches("description==The big picture", &b));
        assert!(
            !view_matches("key==systemlandscape-001", &b),
            "values are case-sensitive"
        );
        assert!(!view_matches("title==Other", &b));
        assert!(view_matches("title!=Other", &b));
        assert!(!view_matches("title!=Landscape", &b));
    }

    #[test]
    fn view_subset_arm_prefix_is_case_insensitive() {
        let b = base();
        assert!(view_matches("Key==SystemLandscape-001", &b));
        assert!(view_matches("TITLE==Landscape", &b));
    }

    #[test]
    fn view_subset_combinators_drop_extra_arms_like_the_reference() {
        let b = base();
        assert!(view_matches(
            "key==SystemLandscape-001 && title==Landscape",
            &b
        ));
        assert!(!view_matches("key==SystemLandscape-001 && title==Nope", &b));
        assert!(view_matches("key==Nope || title==Landscape", &b));
        // Java `split` indexes `[0]`/`[1]` over ALL parts: `a || b || c`
        // evaluates the STANDALONE arms `a` and `b` — the third arm is
        // dropped entirely (here `c` is the matching one, so the result
        // is false even though some arm matches).
        assert!(!view_matches(
            "key==Nope || title==Nope || description==Landscape",
            &b
        ));
        // …and symmetrically a dropped FALSE arm cannot condemn it.
        assert!(view_matches(
            "key==Nope || title==Landscape || description==Nope",
            &b
        ));
        // ` && ` is checked FIRST (reference chain order); its halves
        // are single arms too — `key==Nope` fails, so the conjunction is
        // false no matter what the second half contains.
        assert!(!view_matches(
            "key==Nope && title==Landscape || description==Nope",
            &b
        ));
        // A `||` half is a FULL arm in its own right (negation included)
        // — what the `&&`-first order makes a literal is covered above.
        assert!(view_matches("title==Landscape || key!=Nope", &b));
    }

    #[test]
    fn absent_optional_sides_and_outside_arms_match_nothing() {
        let mut b = base();
        b.title = None;
        b.description = None;
        assert!(!view_matches("title==Landscape", &b), "absent != any value");
        assert!(
            view_matches("title!=Landscape", &b),
            "absent != value is true"
        );
        // outside-subset arms fail soft like the base parser `default:`
        assert!(!view_matches("tags==Element", &b));
        assert!(!view_matches("*", &b));
        assert!(!view_matches("", &b));
    }

    #[test]
    fn formats_render_uniform_rows() {
        let rows = vec![
            Row {
                id: "2".into(),
                class: RowClass::Element,
                kind: "SoftwareSystem".into(),
                name: Some("System".into()),
                tags: vec!["Element".into(), "Software System".into(), "Web".into()],
            },
            Row {
                id: "7".into(),
                class: RowClass::Relationship,
                kind: "Relationship".into(),
                name: Some("User -> Uses -> System".into()),
                tags: vec!["Relationship".into()],
            },
            Row {
                id: "SystemLandscape-001".into(),
                class: RowClass::View,
                kind: "SystemLandscape".into(),
                name: Some("Landscape".into()),
                tags: Vec::new(),
            },
        ];
        assert_eq!(
            String::from_utf8(table_bytes(&rows)).unwrap(),
            "2  SoftwareSystem  System  Element,Software System,Web\n\
             7  Relationship  User -> Uses -> System  Relationship\n\
             SystemLandscape-001  SystemLandscape  Landscape\n"
        );
        assert_eq!(
            String::from_utf8(ids_bytes(&rows)).unwrap(),
            "2\n7\nSystemLandscape-001\n"
        );
        let json: Value =
            serde_json::from_slice(&json_bytes("element.tag==Web", QueryKind::All, &rows).unwrap())
                .unwrap();
        assert_eq!(json["schema"], 1);
        assert_eq!(json["elements"][0]["tags"].as_array().unwrap().len(), 3);
        assert!(json["elements"][0].get("tags").is_some());
        assert!(
            json["views"][0].get("tags").is_none(),
            "views carry no tags field"
        );
        assert_eq!(json["views"][0]["key"], "SystemLandscape-001");
        // the section keys follow --kind
        let json: Value =
            serde_json::from_slice(&json_bytes("*", QueryKind::View, &rows[..1]).unwrap()).unwrap();
        assert!(json.get("elements").is_none());
        assert!(json.get("relationships").is_none());
        assert!(json["views"].as_array().unwrap().is_empty(), "{json}");
    }
}
