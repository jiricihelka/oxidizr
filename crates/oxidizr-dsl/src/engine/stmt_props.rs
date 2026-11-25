//! Chain section: the `tags`/`tag`/`description`/`technology`/`instances`/`url`
//! property statements, the ten `properties` scope arms and their
//! catch-all, the `perspectives` arms with the inline and block forms of
//! `perspective`, the `value`/`description`/`url` statements inside a
//! perspective block, `group` as a COMPONENT property, the `workspace`
//! scope arm, `impliedRelationships` on/off, and the
//! `name`/`description`/`model` statements at workspace scope.
//!
//! Arm order mirrors the official dispatch chain exactly. These property
//! statements all read the RAW tokens (a trailing `{` becomes a tag part, a
//! property value, or an error, depending on the arm) — the brace-dropping
//! view is never used here.

use oxidizr_core::builder::ImpliedRelationshipsStrategy;
use oxidizr_core::model::Perspective;
use oxidizr_core::validate::ElementKind;
use oxidizr_core::{CoreError, Id};

use super::ctx::{Ctx, Holder};
use super::error::DslError;
use super::{Parser, Sec, Stmt, eqi, java_split_str, tok};

#[allow(clippy::too_many_lines)] // the chain arm order IS the contract
pub(crate) fn section(p: &mut Parser, stmt: &Stmt) -> Sec {
    let first = stmt.first();
    let raw = &stmt.tokens;
    // Guards — "top of stack" checks; `no_group` means the TOP groupable
    // context carries no group payload. Fresh `p.top` reads: the official
    // chain re-reads the current context per branch, and every arm that
    // mutates the stack also returns, so each guard observes the same
    // context the old whole-`Ctx` prelude clone snapshotted.
    let no_group = p.top().is_some_and(|c| c.group_payload().is_none());
    let is_item_ng = p.top().is_some_and(super::ctx::Ctx::is_model_item) && no_group;
    let is_element_ng = p.top().is_some_and(Ctx::is_element) && no_group;
    let is_items = p.top().is_some_and(Ctx::is_model_items);
    let is_elements = matches!(p.top(), Some(Ctx::Elements { .. }));
    let is_workspace_ctx = matches!(p.top(), Some(Ctx::Workspace));

    // `tags <tags> [tags]` (`tag` is an alias) — a single model
    // item, or every item of a fan-out context.
    if (eqi(&first, tok::TAGS) || eqi(&first, tok::TAG)) && (is_item_ng || is_items) {
        return (|| {
            let targets = targets(p)?;
            if !raw.includes(1) {
                return Err(DslError::stmt("Expected: tags <tags> [tags]"));
            }
            // EVERY remaining raw token becomes tag material (a trailing
            // `{` becomes a tag part) and each is split on commas.
            let mut parts: Vec<String> = Vec::new();
            for i in 1..raw.size() {
                parts.extend(
                    java_split_str(&raw.get(i), ",")
                        .into_iter()
                        .map(str::to_string),
                );
            }
            for id in &targets {
                p.builder.add_tag_parts(id, &parts);
            }
            Ok(())
        })()
        .into();
    }

    // `description <description>` on an element or element fan-out.
    if eqi(&first, tok::DESCRIPTION) && (is_element_ng || is_elements) {
        return (|| {
            let targets = targets(p)?;
            one_token(raw, "description <description>")?;
            let description = raw.get(1);
            for id in &targets {
                p.builder
                    .set_description(id, &description)
                    .map_err(|e| DslError::core(&e))?;
            }
            Ok(())
        })()
        .into();
    }

    // `technology <technology>` — six context arms in chain order
    // (container and component only when their context carries NO group).
    if eqi(&first, tok::TECHNOLOGY) {
        let single = match p.top() {
            Some(Ctx::Container { group, .. }) if group.is_none() => true,
            // The no-group guard is vacuous for components
            // — no DSL path ever pushes a component context carrying a
            // group (`group` is a component PROPERTY only).
            Some(
                Ctx::Component { .. } | Ctx::DeploymentNode { .. } | Ctx::InfrastructureNode { .. },
            ) => true,
            _ => false,
        };
        if single || is_elements || matches!(p.top(), Some(Ctx::Relationships { .. })) {
            return (|| {
                one_token(raw, "technology <technology>")?;
                let technology = raw.get(1);
                match p.top() {
                    Some(Ctx::Elements { items }) => {
                        let items = items.clone();
                        // In a fan-out, only the four
                        // kinds with a technology field are set; the others
                        // are silently skipped.
                        for id in items {
                            if matches!(
                                p.builder.element_kind(&id),
                                Some(
                                    ElementKind::Container
                                        | ElementKind::Component
                                        | ElementKind::DeploymentNode
                                        | ElementKind::InfrastructureNode
                                )
                            ) {
                                p.builder
                                    .set_technology(&id, &technology)
                                    .map_err(|e| DslError::core(&e))?;
                            }
                        }
                    }
                    Some(Ctx::Relationships { items }) => {
                        let items = items.clone();
                        // In a relationships fan-out — only
                        // relationships between two instances are set.
                        let is_instance = |kind: Option<ElementKind>| {
                            matches!(
                                kind,
                                Some(
                                    ElementKind::SoftwareSystemInstance
                                        | ElementKind::ContainerInstance
                                )
                            )
                        };
                        for id in items {
                            let endpoints = p
                                .builder
                                .workspace()
                                .model
                                .relationships
                                .iter()
                                .find(|r| r.id == id)
                                .map(|r| (r.source.clone(), r.destination.clone()));
                            let Some((src, dst)) = endpoints else {
                                continue;
                            };
                            // Read both kinds before the mutable borrow.
                            let (src_kind, dst_kind) =
                                (p.builder.element_kind(&src), p.builder.element_kind(&dst));
                            if is_instance(src_kind) && is_instance(dst_kind) {
                                p.builder
                                    .set_relationship_technology(&id, &technology)
                                    .map_err(|e| DslError::core(&e))?;
                            }
                        }
                    }
                    _ => {
                        let targets = targets(p)?;
                        for id in &targets {
                            p.builder
                                .set_technology(id, &technology)
                                .map_err(|e| DslError::core(&e))?;
                        }
                    }
                }
                Ok(())
            })()
            .into();
        }
        // No technology arm's context guard matched — fall through (the
        // statement may yet hit a later arm or the catch-all).
    }

    // `instances <number|range>` on a deployment node — the
    // official tooling stores the RAW string with no validation.
    if eqi(&first, tok::INSTANCES) && matches!(p.top(), Some(Ctx::DeploymentNode { .. })) {
        return (|| {
            let id = item_id(p)?;
            one_token(raw, "instances <number|range>")?;
            p.builder
                .set_instances(&id, &raw.get(1))
                .map_err(|e| DslError::core(&e))
        })()
        .into();
    }

    // `url <url>` — model item or fan-out.
    if eqi(&first, tok::URL) && (is_item_ng || is_items) {
        return (|| {
            let targets = targets(p)?;
            one_token(raw, "url <url>")?;
            let url = raw.get(1);
            for id in &targets {
                p.builder
                    .set_url(id, &url)
                    .map_err(|e| DslError::core(&e))?;
            }
            Ok(())
        })()
        .into();
    }

    // `properties` — ten context arms, each pushing a
    // properties context OVER ITS HOLDER(S). The push is UNCONDITIONAL
    // (`{` is neither required nor optional — extra tokens are ignored, a
    // missing `{` still opens the context).
    if eqi(&first, tok::PROPERTIES)
        && let Some(holder) = p.top().and_then(properties_holder)
    {
        p.start_context(Ctx::Properties {
            holders: vec![holder],
        });
        return Sec::Handled;
    }

    // Inside a properties block ANY statement is a `<name> <value>`
    // pair, read over the RAW tokens.
    if let Some(Ctx::Properties { holders }) = p.top() {
        let holders = holders.clone();
        return (|| {
            if raw.has_more_than(1) {
                return Err(DslError::stmt("Too many tokens, expected: <name> <value>"));
            }
            if raw.size() != 2 {
                return Err(DslError::stmt("Expected: <name> <value>"));
            }
            let name = raw.get(0);
            let value = raw.get(1);
            for h in &holders {
                p.apply_property(h, &name, &value);
            }
            Ok(())
        })()
        .into();
    }

    // `perspectives` — push over the item/fan-out holders
    // (unconditional push, as for `properties`).
    if eqi(&first, tok::PERSPECTIVES) && (is_item_ng || is_items) {
        return (|| {
            let holders = targets(p)?
                .into_iter()
                .map(Holder::Item)
                .collect::<Vec<_>>();
            // A fan-out is ONE `Items` holder covering every
            // item (the list is applied per item), not one holder per item.
            let holders = match p.top() {
                Some(Ctx::Elements { items } | Ctx::Relationships { items }) => {
                    vec![Holder::Items(items.clone())]
                }
                _ => holders,
            };
            p.start_context(Ctx::Perspectives { holders });
            Ok(())
        })()
        .into();
    }

    // `perspective <name> {` — a NAMED perspective block; requires the
    // brace AND exactly three RAW tokens, else the statement falls through
    // to the inline form below (`perspective X Y` defines a perspective
    // CALLED "perspective" — the official tooling's quirk, preserved).
    if eqi(&first, tok::PERSPECTIVE)
        && matches!(p.top(), Some(Ctx::Perspectives { .. }))
        && Parser::should_start_context(raw)
        && raw.size() == 3
    {
        p.start_context(Ctx::Perspective {
            // The one-argument perspective constructor performs NO name
            // validation; the block close adds via the OBJECT path, which
            // only duplicate-checks.
            def: Perspective {
                name: raw.get(1).into_owned(),
                description: None,
                value: None,
                url: None,
            },
        });
        return Sec::Handled;
    }

    // Inside a perspectives block, `<name> <description> [value]`
    // defines a perspective inline.
    if let Some(Ctx::Perspectives { holders }) = p.top() {
        let holders = holders.clone();
        return (|| {
            if raw.has_more_than(2) {
                return Err(DslError::stmt(
                    "Too many tokens, expected: <name> <description> [value]",
                ));
            }
            if !raw.includes(1) {
                return Err(DslError::stmt("Expected: <name> <description> [value]"));
            }
            let name = raw.get(0).into_owned();
            let description = raw.get(1).into_owned();
            let value = if raw.includes(2) {
                raw.get(2).into_owned()
            } else {
                String::default()
            };
            // Adding a perspective validates the name (blank is
            // rejected) BEFORE any holder is touched.
            if name.trim().is_empty() {
                return Err(DslError::core(&CoreError::PerspectiveNameRequired));
            }
            p.add_perspective_to_holders(
                &holders,
                &Perspective {
                    name,
                    description: Some(description),
                    value: Some(value),
                    url: None,
                },
            )
        })()
        .into();
    }

    // `value`/`description`/`url` statements inside a perspective
    // block (plain setters — empty strings land as-is).
    if matches!(p.top(), Some(Ctx::Perspective { .. })) {
        let field = if eqi(&first, tok::PERSPECTIVE_VALUE) {
            Some(PerspectiveField::Value)
        } else if eqi(&first, tok::PERSPECTIVE_DESCRIPTION) {
            Some(PerspectiveField::Description)
        } else if eqi(&first, tok::PERSPECTIVE_URL) {
            Some(PerspectiveField::Url)
        } else {
            None
        };
        if let Some(field) = field {
            return (|| {
                let grammar = match field {
                    PerspectiveField::Value => "value <value>",
                    PerspectiveField::Description => "description <description>",
                    PerspectiveField::Url => "url <url>",
                };
                one_token(raw, grammar)?;
                let value = raw.get(1).into_owned();
                if let Some(Ctx::Perspective { def }) = p.stack.last_mut() {
                    match field {
                        PerspectiveField::Value => def.value = Some(value),
                        PerspectiveField::Description => def.description = Some(value),
                        PerspectiveField::Url => def.url = Some(value),
                    }
                }
                Ok(())
            })()
            .into();
        }
    }

    // `group <name>` — a component PROPERTY, read over RAW
    // tokens: a trailing `{` is "Too many tokens".
    // An existing group makes this a NESTED group: the separator property
    // joins `existing + separator + name`.
    if eqi(&first, tok::GROUP) && matches!(p.top(), Some(Ctx::Component { .. })) {
        return (|| {
            if raw.includes(2) {
                return Err(DslError::stmt("Too many tokens, expected: group <name>"));
            }
            if !raw.includes(1) {
                return Err(DslError::stmt("Expected: group <name>"));
            }
            let id = item_id(p)?;
            let name = raw.get(1);
            let existing = p
                .builder
                .element_mut(&id)
                .and_then(|el| el.group().map(str::to_owned));
            let group = match existing {
                Some(existing) if !existing.trim().is_empty() => {
                    let separator = p
                        .builder
                        .workspace()
                        .model
                        .properties
                        .get("structurizr.groupSeparator")
                        .unwrap_or_default();
                    if separator.trim().is_empty() {
                        return Err(DslError::stmt(
                            "To use nested groups, please define a model property named structurizr.groupSeparator",
                        ));
                    }
                    format!("{existing}{separator}{name}")
                }
                _ => name.into_owned(),
            };
            if let Some(mut el) = p.builder.element_mut(&id) {
                el.set_group(Some(group));
            }
            Ok(())
        })()
        .into();
    }

    // `workspace [name] [description]` / `workspace extends …` —
    // only on an EMPTY context stack, and only once.
    if eqi(&first, tok::WORKSPACE) && p.stack.is_empty() {
        return (|| {
            if p.parsed_tokens.iter().any(|t| t == tok::WORKSPACE) {
                return Err(DslError::stmt(
                    "Multiple workspaces are not permitted in a DSL definition",
                ));
            }
 // The workspace statement reads tokens with any trailing `{`
 // dropped.
            let w = raw.without_context_start_token();
            if w.has_more_than(2) {
                return Err(DslError::stmt(
                    "Too many tokens, expected: workspace [name] [description] or workspace extends <file|url>",
                ));
            }
 // Default name/description — the DSL layer applies
 // them (core keeps them unset for JSON).
            p.builder.set_workspace_name("Name");
            p.builder.set_workspace_description("Description");
            if w.includes(1) {
                let source = w.get(1);
 // `extends` is compared CASE-SENSITIVELY.
                if source == tok::EXTENDS {
                    if !w.includes(2) {
                        return Err(DslError::stmt("Expected: workspace extends <file|url>"));
                    }
                    let base = w.get(2);
                    p.extend_from(&base)?;
                } else {
                    p.builder.set_workspace_name(source);
                    if w.includes(2) {
                        p.builder.set_workspace_description(w.get(2));
                    }
                }
            }
 // Captured AFTER the workspace statement,
 // read later when contexts start.
            let model = &p.builder.workspace().model;
            p.extending_workspace = !model.people.is_empty()
                || !model.software_systems.is_empty()
                || !model.deployment_nodes.is_empty();
 // DSL workspaces get the
 // UnlessAnyRelationshipExists strategy unconditionally.
            p.builder
                .set_implied_strategy(ImpliedRelationshipsStrategy::UnlessAnyRelationshipExists);
            p.have_workspace = true;
            p.parsed_tokens.push(tok::WORKSPACE.to_string());
            p.start_context(Ctx::Workspace);
            Ok(())
        })()
        .into();
    }

    // `impliedRelationships`/`!impliedRelationships <true|false|fqcn>`
    // — any context; on an empty stack the missing context fails first.
    if eqi(&first, tok::IMPLIED_RELATIONSHIPS) || eqi(&first, &tok::IMPLIED_RELATIONSHIPS[1..]) {
        return (|| {
            if p.top().is_none() {
                return Err(DslError::stmt("Unexpected end of context"));
            }
            if raw.has_more_than(1) {
                return Err(DslError::stmt(
                    "Too many tokens, expected: !impliedRelationships <true|false|fqcn>",
                ));
            }
            if !raw.includes(1) {
                return Err(DslError::stmt(
                    "Expected: !impliedRelationships <true|false|fqcn>",
                ));
            }
            let option = raw.get(1);
            if eqi(&option, "false") {
                p.builder
                    .set_implied_strategy(ImpliedRelationshipsStrategy::Default);
                return Ok(());
            }
            if eqi(&option, "true") {
                p.builder.set_implied_strategy(
                    ImpliedRelationshipsStrategy::UnlessAnyRelationshipExists,
                );
                return Ok(());
            }
 // The three built-in strategy names load natively; any other
 // FQCN needs the official tooling's plugin loader — a plugin-only reject, and the official
 // tooling's class-not-found text is the message.
            let strategy = match &*option {
                "com.structurizr.model.DefaultImpliedRelationshipsStrategy" => {
                    ImpliedRelationshipsStrategy::Default
                }
                "com.structurizr.model.CreateImpliedRelationshipsUnlessAnyRelationshipExistsStrategy" => {
                    ImpliedRelationshipsStrategy::UnlessAnyRelationshipExists
                }
                "com.structurizr.model.CreateImpliedRelationshipsUnlessSameRelationshipExistsStrategy" => {
                    ImpliedRelationshipsStrategy::UnlessSameRelationshipExists
                }
                _ => {
                    return Err(DslError::coded(
                        oxidizr_diag::codes::JVM_ONLY,
                        format!("Error loading implied relationships strategy: {option} was not found"),
                    ));
                }
            };
            p.builder.set_implied_strategy(strategy);
            Ok(())
        })()
        .into();
    }

    // `name <name>` at workspace scope.
    if eqi(&first, tok::NAME) && is_workspace_ctx {
        return (|| {
            one_token(raw, "name <name>")?;
            p.builder.set_workspace_name(raw.get(1));
            Ok(())
        })()
        .into();
    }

    // `description <description>` at workspace scope.
    if eqi(&first, tok::DESCRIPTION) && is_workspace_ctx {
        return (|| {
            one_token(raw, "description <description>")?;
            p.builder.set_workspace_description(raw.get(1));
            Ok(())
        })()
        .into();
    }

    // `model { … }` at workspace scope — once only; the push is
    // unconditional (no `{` needed).
    if eqi(&first, tok::MODEL) && is_workspace_ctx {
        return (|| {
            if p.parsed_tokens.iter().any(|t| t == tok::MODEL) {
                return Err(DslError::stmt(
                    "Multiple models are not permitted in a DSL definition",
                ));
            }
            p.parsed_tokens.push(tok::MODEL.to_string());
            p.start_context(Ctx::Model { group: None });
            Ok(())
        })()
        .into();
    }

    Sec::No
}

/// Which perspective field a statement inside a perspective block
/// sets.
enum PerspectiveField {
    Value,
    Description,
    Url,
}

/// The single target (model item) of the top context —
/// elements AND relationships alike.
fn item_id(p: &Parser<'_>) -> Result<Id, DslError> {
    p.top()
        .and_then(Ctx::model_item_id)
        .cloned()
        .ok_or_else(|| DslError::stmt("Unexpected end of context"))
}

/// The item targets: the fan-out contexts contribute ALL their items, a
/// model-item context its one item.
fn targets(p: &Parser<'_>) -> Result<Vec<Id>, DslError> {
    match p.top() {
        Some(Ctx::Elements { items } | Ctx::Relationships { items }) => Ok(items.clone()),
        _ => Ok(vec![item_id(p)?]),
    }
}

/// The standard two-grammar-token guard used by the single-value
/// statements: `has_more_than(1)` then `includes(1)`, over the RAW tokens.
fn one_token(raw: &super::Tokens, grammar: &str) -> Result<(), DslError> {
    if raw.has_more_than(1) {
        return Err(DslError::stmt(format!(
            "Too many tokens, expected: {grammar}"
        )));
    }
    if !raw.includes(1) {
        return Err(DslError::stmt(format!("Expected: {grammar}")));
    }
    Ok(())
}

/// The holder of the ten `properties` scope arms — a properties
/// context over one holder (the fan-out contexts pass their whole list).
/// The configuration arm holds the WORKSPACE, and the views arm the
/// view-set configuration.
#[allow(clippy::match_same_arms)] // the Configuration and Views holders collapse identically
fn properties_holder(top: &Ctx) -> Option<Holder> {
    match top {
        Ctx::Workspace => Some(Holder::Workspace),
        Ctx::Model { .. } => Some(Holder::Model),
        Ctx::Configuration => Some(Holder::Workspace),
        c if c.is_model_item() && c.group_payload().is_none() => {
            c.model_item_id().map(|id| Holder::Item(id.clone()))
        }
        Ctx::Elements { items } | Ctx::Relationships { items } => {
            Some(Holder::Items(items.clone()))
        }
        Ctx::Views => Some(Holder::ViewSet),
        // All view contexts, including the parallel-dynamic
        // variant (a kind of dynamic view) …
        Ctx::LandscapeView { view }
        | Ctx::ContextView { view }
        | Ctx::ContainerView { view }
        | Ctx::ComponentView { view }
        | Ctx::FilteredView { view }
        | Ctx::DynamicView { view }
        | Ctx::DynamicParallel { view, .. }
        | Ctx::DeploymentView { view }
        | Ctx::CustomView { view }
        | Ctx::ImageView { view, .. } => Some(Holder::View(*view)),
        // … but NOT the dynamic-relationship context,
        // whose own arm holds the RELATIONSHIP VIEW.
        Ctx::DynamicRel { view, rel } => Some(Holder::DynamicRel {
            view: *view,
            rel: rel.clone(),
        }),
        Ctx::ElementStyle { idx } => Some(Holder::ElementStyle { idx: *idx }),
        Ctx::RelationshipStyle { idx } => Some(Holder::RelationshipStyle { idx: *idx }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::super::{ParseOptions, ParseResult, parse_dsl};
    use super::*;

    /// Whole-file harness: the `workspace`/`model` scope arms need the real
    /// entry from an empty starting stack.
    fn full(dsl: &str) -> ParseResult {
        parse_dsl(dsl, ParseOptions::fragment()).expect("parse")
    }

    /// Model-scope harness (mirrors the `stmt_model` tests).
    fn in_model(dsl: &str) -> ParseResult {
        let mut p = Parser::new(ParseOptions::fragment());
        p.start_context(Ctx::Model { group: None });
        let logical = crate::preprocess::preprocess(dsl);
        p.parse_lines(&logical, dsl, false);
        assert_eq!(p.stack.len(), 1, "unbalanced contexts: {:?}", p.errors);
        p.stack.pop().expect("seeded context");
        p.finish()
    }

    fn errors(r: &ParseResult) -> Vec<String> {
        r.errors.iter().map(|e| e.message.clone()).collect()
    }

    fn assert_clean(r: &ParseResult) {
        assert!(r.errors.is_empty(), "unexpected errors: {:?}", errors(r));
    }

    #[test]
    fn tags_description_url_on_elements_fanout() {
        let r = in_model(
            r#"
s1 = softwareSystem "S1" "keep"
s2 = softwareSystem "S2" "keep" "TagA"
!elements element.tag==TagA {
  tags "T1,T2" "T3"
  description "Fan desc"
  technology "Ignored"
  url "http://fan"
}
"#,
        );
        assert_clean(&r);
        let w = &r.workspace;
        let s1 = &w.model.software_systems[0];
        let s2 = &w.model.software_systems[1];
        // Stored tags carry the EXPLICIT parts only — the implicit
        // `Element`/`Software System` stack is computed at write time.
        assert!(s1.item.tags.stored().is_empty(), "expression misses S1");
        assert_eq!(
            s2.item.tags.stored(),
            ["TagA", "T1", "T2", "T3"],
            "ModelItemsParser fans out to every item"
        );
        assert_eq!(s2.item.description.as_deref(), Some("Fan desc"));
        assert_eq!(s2.item.url.as_deref(), Some("http://fan"));
        // `technology` is silently skipped for systems — the core model has
        // no technology field on `SoftwareSystem` to check.
    }

    #[test]
    fn rel_block_accepts_tags_url_not_description() {
        // A relationship block permits tags/url/properties/
        // perspectives only — `description` has no arm there.
        let r = in_model(
            r#"
a = softwareSystem "A"
b = softwareSystem "B"
r = a -> b "Uses" {
  tags "RT"
  url "http://rel"
  description "Nope"
}
"#,
        );
        assert_eq!(
            errors(&r),
            vec!["Unexpected tokens (expected: tags, url, properties, perspectives)"]
        );
        let rel = &r.workspace.model.relationships[0];
        assert_eq!(rel.tags.stored(), ["RT"]);
        assert_eq!(rel.url.as_deref(), Some("http://rel"));
        assert_eq!(rel.description.as_deref(), Some("Uses"), "not overwritten");
    }

    #[test]
    fn tags_with_trailing_brace_adds_it_as_a_tag() {
        // Property arms read the RAW tokens: the `{` is tag #3 and NO
        // context opens, so the lone `}` closes the element.
        let r = in_model(
            r#"
s = softwareSystem "S" {
  tags Foo {
}
"#,
        );
        assert_clean(&r);
        assert_eq!(
            r.workspace.model.software_systems[0].item.tags.stored(),
            ["Foo", "{"]
        );
    }

    #[test]
    fn property_grammar_errors() {
        let r = in_model(
            r#"
s = softwareSystem "S" {
  properties {
    a b c
  }
}
t = softwareSystem "T" {
  properties {
    lonely
}
}
"#,
        );
        assert_eq!(
            errors(&r),
            vec![
                "Too many tokens, expected: <name> <value>",
                "Expected: <name> <value>",
            ]
        );
    }

    #[test]
    fn properties_scopes_and_overwrite() {
        let r = full(
            r#"
workspace {
  properties {
    "wk" "wv"
  }
  model {
    properties {
      "mk" "mv"
    }
    s = softwareSystem "S" {
      properties {
        "ek" "ev"
        "ek" "ev2"
      }
    }
  }
}
"#,
        );
        assert_clean(&r);
        let w = &r.workspace;
        assert_eq!(w.properties.get("wk"), Some("wv"));
        assert_eq!(w.model.properties.get("mk"), Some("mv"));
        assert_eq!(
            w.model.software_systems[0].item.properties.get("ek"),
            Some("ev2"),
            "LinkedHashMap.put: last value wins"
        );
    }

    #[test]
    fn properties_context_push_needs_no_brace() {
        // The push is UNCONDITIONAL — a `properties` statement without a
        // `{` still opens the context (extra tokens would be ignored).
        let r = in_model(
            r#"
s = softwareSystem "S" {
  properties
  a b
}
}
"#,
        );
        assert_clean(&r);
        assert_eq!(
            r.workspace.model.software_systems[0]
                .item
                .properties
                .get("a"),
            Some("b")
        );
    }

    #[test]
    fn technology_sets_kind_bearing_elements() {
        let r = in_model(
            r#"
s = softwareSystem "S" {
  c = container "C" {
    technology "Spring Boot"
    k = component "K" {
      technology "Spring MVC"
    }
  }
}
p = person "P" {
  technology "Nope"
}
"#,
        );
        assert_eq!(errors(&r).len(), 1, "person has no technology arm");
        assert!(errors(&r)[0].starts_with("Unexpected tokens"));
        let w = &r.workspace;
        let c = &w.model.software_systems[0].containers[0];
        assert_eq!(c.technology.as_deref(), Some("Spring Boot"));
        assert_eq!(c.components[0].technology.as_deref(), Some("Spring MVC"));
    }

    #[test]
    fn technology_grammar_errors() {
        let r = in_model(
            r#"
s = softwareSystem "S" {
  c = container "C" {
    technology
    technology A B
  }
}
"#,
        );
        assert_eq!(
            errors(&r),
            vec![
                "Expected: technology <technology>",
                "Too many tokens, expected: technology <technology>",
            ]
        );
    }

    #[test]
    fn perspectives_inline_and_block_forms() {
        let r = in_model(
            r#"
s = softwareSystem "S" {
  perspectives {
    "Security" "High risk" "H"
    perspective "Debt" {
      description "Tech debt"
      value "M"
      url "http://d"
    }
    perspective Q "no brace"
  }
}
"#,
        );
        assert_clean(&r);
        let ps = &r.workspace.model.software_systems[0].item.perspectives;
        assert_eq!(ps.len(), 3);
        assert_eq!(ps[0].name, "Security");
        assert_eq!(ps[0].description.as_deref(), Some("High risk"));
        assert_eq!(ps[0].value.as_deref(), Some("H"), "3-arg value");
        assert_eq!(ps[1].name, "Debt");
        assert_eq!(ps[1].description.as_deref(), Some("Tech debt"));
        assert_eq!(ps[1].value.as_deref(), Some("M"));
        assert_eq!(ps[1].url.as_deref(), Some("http://d"));
        // `perspective Q "no brace"` falls to the inline parser: the name is
        // the FIRST token — literally "perspective" (official quirk, kept).
        assert_eq!(ps[2].name, "perspective");
        assert_eq!(ps[2].description.as_deref(), Some("Q"));
        assert_eq!(ps[2].value.as_deref(), Some("no brace"));
    }

    #[test]
    fn perspective_errors() {
        let r = in_model(
            r#"
s = softwareSystem "S" {
  perspectives {
    "P" "first"
    perspective "P" {
    }
    " " "blank name"
    "one" "two" "three" "four"
    alone
  }
}
"#,
        );
        assert_eq!(
            errors(&r),
            vec![
                // block close: object-path duplicate guard
                "A perspective named \"P\" already exists.",
                "Perspective name must be specified",
                "Too many tokens, expected: <name> <description> [value]",
                "Expected: <name> <description> [value]",
            ]
        );
        assert_eq!(
            r.workspace.model.software_systems[0]
                .item
                .perspectives
                .len(),
            1
        );
    }

    #[test]
    fn perspective_block_fields_and_catch_all() {
        let r = in_model(
            r#"
s = softwareSystem "S" {
  perspectives {
    perspective "P" {
      value
      url a b
      nonsense x
    }
  }
}
"#,
        );
        assert_eq!(
            errors(&r),
            vec![
                "Expected: value <value>",
                "Too many tokens, expected: url <url>",
                // The perspective block permits description and value only.
                "Unexpected tokens (expected: description, value)",
            ]
        );
    }

    #[test]
    fn group_property_on_components() {
        let r = in_model(
            r#"
s = softwareSystem "S" {
  c = container "C" {
    k = component "K" {
      group "A"
      group "B"
    }
  }
}
"#,
        );
        // No separator property: the SECOND group statement fails; the
        // component keeps "A". (`group` is a component PROPERTY only.)
        assert_eq!(
            errors(&r).last().map(String::as_str),
            Some(
                "To use nested groups, please define a model property named structurizr.groupSeparator"
            )
        );
        let k = &r.workspace.model.software_systems[0].containers[0].components[0];
        assert_eq!(k.group.as_deref(), Some("A"));
    }

    #[test]
    fn group_property_joins_with_separator() {
        let r = full(
            r#"
workspace {
  model {
    properties {
      "structurizr.groupSeparator" "/"
    }
    s = softwareSystem "S" {
      c = container "C" {
        k = component "K" {
          group "A"
          group "B"
          group "C" "extra"
        }
      }
    }
  }
}
"#,
        );
        assert_eq!(errors(&r), vec!["Too many tokens, expected: group <name>"]);
        let k = &r.workspace.model.software_systems[0].containers[0].components[0];
        assert_eq!(k.group.as_deref(), Some("A/B"), "joined with the separator");
    }

    #[test]
    fn workspace_scope_name_description_and_defaults() {
        let r = full(
            r#"
workspace "My W" "My D" {
  name "Renamed"
  description "Re-described"
  model {
  }
}
"#,
        );
        assert_clean(&r);
        assert_eq!(r.workspace.name.as_deref(), Some("Renamed"));
        assert_eq!(r.workspace.description.as_deref(), Some("Re-described"));

        let d = full("workspace {\n}\n");
        assert_clean(&d);
        assert_eq!(d.workspace.name.as_deref(), Some("Name"));
        assert_eq!(d.workspace.description.as_deref(), Some("Description"));
    }

    #[test]
    fn workspace_grammar_and_dup_guards() {
        let r = full("workspace A B C\n");
        assert_eq!(
            errors(&r),
            vec![
                "Too many tokens, expected: workspace [name] [description] or workspace extends <file|url>"
            ]
        );

        let r = full("workspace \"A\" {\n}\nworkspace \"B\" {\n}\n");
        // The failed second `workspace` pushes no context, so its closing
        // `}` lands on an EMPTY stack — `endContext` throws.
        assert_eq!(
            errors(&r),
            vec![
                "Multiple workspaces are not permitted in a DSL definition",
                "Unexpected end of context"
            ]
        );

        let r = full("workspace {\n  model {\n  }\n  model {\n  }\n}\n");
        // Same shape: the `}` that would close the failed model instead
        // closes the workspace, and the last `}` lands on an empty stack.
        assert_eq!(
            errors(&r),
            vec![
                "Multiple models are not permitted in a DSL definition",
                "Unexpected end of context"
            ]
        );

        // `extends` compares CASE-SENSITIVELY: `Extends` is just a name.
        let r = full("workspace Extends base.dsl {\n}\n");
        assert_clean(&r);
        assert_eq!(r.workspace.name.as_deref(), Some("Extends"));
        assert_eq!(r.workspace.description.as_deref(), Some("base.dsl"));
    }

    #[test]
    fn workspace_extends_needs_a_source_and_skips_without_a_file() {
        // The fragment parse has no file: the file branch is skipped
        // entirely — the fresh "Name"/"Description" workspace
        // stands, no error, no portability change.
        let r = full("workspace extends base.dsl {\n}\n");
        assert_clean(&r);
        assert_eq!(r.workspace.name.as_deref(), Some("Name"));

        let r = full("workspace extends\n");
        assert_eq!(errors(&r), vec!["Expected: workspace extends <file|url>"]);

        // Network stays OXZ-5001 even without a file (the URL guard precedes
        // the file guard).
        let r = full("workspace extends https://example.invalid/x.dsl\n");
        assert_eq!(r.errors[0].code, oxidizr_diag::codes::NET_DISABLED);
        assert_eq!(r.errors[0].message, "Extends via HTTPS are not permitted");
    }

    #[test]
    fn implied_relationships_toggle() {
        // The default implied-relationships strategy propagates UP the
        // parent chains: `p -> c` additionally creates `p -> s` when the DSL
        // workspace default (UnlessAnyRelationshipExists) is active.
        let base = r#"
workspace {
  model {
    %TOGGLE%
    p = person "P"
    s = softwareSystem "S" {
      c = container "C"
    }
    p -> c "Uses"
  }
}
"#;
        let r = full(&base.replace("%TOGGLE%", ""));
        assert_clean(&r);
        assert_eq!(r.workspace.model.relationships.len(), 2);
        assert!(
            r.workspace.model.relationships.iter().any(|rel| {
                rel.description.as_deref() == Some("Uses") && rel.linked_relationship_id.is_some()
            }),
            "the implied p -> s carries the linked relationship id"
        );

        let r = full(&base.replace("%TOGGLE%", "!impliedRelationships false"));
        assert_clean(&r);
        assert_eq!(r.workspace.model.relationships.len(), 1);

        // The bare (no `!`) spelling works too.
        let r = full(&base.replace("%TOGGLE%", "impliedRelationships false"));
        assert_clean(&r);
        assert_eq!(r.workspace.model.relationships.len(), 1);

        let r = full(&base.replace("%TOGGLE%", "!impliedRelationships com.example.Foo"));
        assert_eq!(
            errors(&r),
            vec!["Error loading implied relationships strategy: com.example.Foo was not found"]
        );
        assert_eq!(r.errors[0].code, oxidizr_diag::codes::JVM_ONLY);
    }

    #[test]
    fn implied_relationships_on_empty_stack() {
        // The current context is resolved BEFORE the statement runs —
        // an empty stack fails with the context error.
        let r = full("!impliedRelationships true\n");
        assert_eq!(errors(&r), vec!["Unexpected end of context"]);
    }
}
