//! `query` evaluator entry (§P4-M3 O-1, F6): the DSL expression grammar
//! IS the query language (P4-D3, corrected 2026-11-21 — no `Predicate`
//! type exists; the pinned `ExpressionParser` port in `engine/expr.rs`
//! is the single evaluator, and this module is its second consumer after
//! the view `include`/`exclude` statements).
//!
//! The entry evaluates `ExprScope::Model` — the base `ExpressionParser`
//! domain (identifiers via the `structurizr.dsl.identifier` register,
//! `Tag ==`/`Tag !=`, `Technology ==`/`!=`, `Property ==`, `Type ==`,
//! `Group ==`, `Parent ==`, `->` coupling, `afferent`/`efferent`,
//! ` && `/` || ` combinations) — over a LOADED workspace, without
//! re-parsing any source. The workspace enters through
//! [`oxidizr_core::WorkspaceBuilder::adopt`] and the identifiers carried
//! in `structurizr.dsl.identifier` properties register exactly like the
//! `workspace extends` JSON-base leg (`WorkspaceParser.registerIdentifiers`),
//! so an expression resolves the same against a `.dsl` and its JSON twin.

use oxidizr_core::configuration::Workspace;
use oxidizr_core::ids::Id;

use crate::engine::expr::{ExprScope, Mi};
use crate::engine::{DslError, ParseOptions, Parser};

/// The matched model items, in engine result order (`Set` semantics:
/// `||` unions deduplicate first-wins, `&&` intersects — engine
/// `union`/`intersect`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QueryResult {
    /// Matching elements.
    pub elements: Vec<Id>,
    /// Matching relationships.
    pub relationships: Vec<Id>,
}

/// Evaluate `expr` over `workspace` (the base `ExpressionParser` domain).
///
/// The unhandled-expression `default:` arm of the reference base parser
/// ADDS NOTHING (no throw), so an expression no arm matches evaluates to
/// an EMPTY result — callers surface zero matches, not an error. The
/// throwing arms (unknown identifier, `->` with no such element, …)
/// surface as a statement-level [`DslError`] with the reference message.
///
/// # Errors
///
/// Returns the reference-shaped statement error when a throwing grammar
/// arm fires (e.g. `The element "x" does not exist`), or when two
/// elements/relationships carry the same `structurizr.dsl.identifier`
/// (the register rejects duplicates, mirroring `registerIdentifiers`).
pub fn run(workspace: &Workspace, expr: &str) -> Result<QueryResult, DslError> {
    // FS/env-free options: the query never reads files or constants.
    let mut parser = Parser::new(ParseOptions::fragment());
    parser.builder.adopt(workspace.clone());
    parser.register_json_identifiers()?;
    let items = parser.parse_expression(expr, ExprScope::Model)?;
    let mut out = QueryResult::default();
    for item in items {
        match item {
            Mi::Element(id) => out.elements.push(id),
            Mi::Rel(id) => out.relationships.push(id),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::engine::{ParseOptions, parse_dsl};
    use oxidizr_core::configuration::Workspace;

    fn ws(source: &str) -> Workspace {
        parse_dsl(source, ParseOptions::fragment())
            .expect("parses")
            .workspace
    }

    const MODEL: &str = r#"
        workspace {
            model {
                user = person "User"
                system = softwareSystem "System" {
                    tags "Web"
                    api = container "API" {
                        technology "Spring"
                    }
                }
                other = softwareSystem "Other"
                user -> system "Uses"
                system -> other "Reads from"
            }
        }
    "#;

    fn el_id(ws: &Workspace, name: &str) -> oxidizr_core::ids::Id {
        for p in &ws.model.people {
            if p.item.name == name {
                return p.item.id.clone();
            }
        }
        for s in &ws.model.software_systems {
            if s.item.name == name {
                return s.item.id.clone();
            }
        }
        panic!("no element {name}");
    }

    #[test]
    fn identifier_arm_resolves_registered_identifiers() {
        let w = ws(MODEL);
        let r = run(&w, "user").unwrap();
        assert_eq!(r.elements, vec![el_id(&w, "User")]);
        assert!(r.relationships.is_empty(), "{:?}", r.relationships);
    }

    #[test]
    fn tag_technology_and_type_arms() {
        let w = ws(MODEL);
        // expression VALUES are bare tokens (reference tests:
        // `element.tag==Container`) — no quoting in the grammar.
        assert_eq!(
            run(&w, "element.tag==Web").unwrap().elements,
            vec![el_id(&w, "System")]
        );
        // technology lives on containers (not systems) — the reference's
        // five-stream technology filter.
        let api = w.model.software_systems[0].containers[0].item.id.clone();
        assert_eq!(
            run(&w, "element.technology==Spring").unwrap().elements,
            vec![api]
        );
        // the type token is the ENUM spelling (space-free): the reference
        // matches the lowercased Java type name (`softwaresystem`).
        let systems = run(&w, "element.type==SoftwareSystem").unwrap();
        assert_eq!(systems.elements.len(), 2);
        assert!(
            systems.relationships.is_empty(),
            "{:?}",
            systems.relationships
        );
    }

    #[test]
    fn relationship_coupling_arm() {
        let w = ws(MODEL);
        let r = run(&w, "user->system").unwrap();
        assert!(r.elements.is_empty(), "{:?}", r.elements);
        // the ONE relationship between User and System
        assert_eq!(r.relationships.len(), 1);
        let rel = w
            .model
            .relationships
            .iter()
            .find(|rel| rel.description.as_deref() == Some("Uses"))
            .expect("Uses rel");
        assert_eq!(r.relationships, vec![rel.id.clone()]);
    }

    #[test]
    fn composition_arms() {
        let w = ws(MODEL);
        let both = run(&w, "element.tag==Web || element.tag==Web").unwrap();
        assert_eq!(both.elements.len(), 1, "union dedupes");
        let none = run(&w, "element.tag==Web && element.tag==Other").unwrap();
        assert!(none.elements.is_empty(), "intersect keeps only shared");
    }

    #[test]
    fn unmatched_expressions_are_empty_not_errors() {
        let w = ws(MODEL);
        // base-parser `default:` adds nothing — an unregistered identifier
        // and a nonsense token both resolve to EMPTY (no throw).
        assert_eq!(run(&w, "nobody").unwrap(), super::QueryResult::default());
        assert_eq!(
            run(&w, "widget frobnicate").unwrap(),
            super::QueryResult::default()
        );
    }
}
