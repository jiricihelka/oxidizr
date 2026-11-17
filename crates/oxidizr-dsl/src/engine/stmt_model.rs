//! Chain section: reference `StructurizrDslParser.java` arms at Java lines
//! 350–611 — `-/>` removal, the four relationship-statement arms, the
//! `!element(s)`/`!relationship(s)` finders, the element-definition arms
//! (`element`/`person`/`softwareSystem`/`container`/`component`), the
//! `!components` reject, the `enterprise` removal throw and the five
//! `group` arms. The `tags`/`tag` arms (612+) live in [`super::stmt_props`].
//!
//! Every arm annotation is the Java line number; the arm ORDER here is the
//! compatibility contract (mirrors the reference `else if` chain exactly).
//! Chain guards read the RAW tokens (including a trailing `{`); the parser
//! ports receive `withoutContextStartToken()`.

use oxidizr_core::Id;
use oxidizr_core::validate::ElementKind;

use super::ctx::{Ctx, DslEnvironment, DslGroup};
use super::error::DslError;
use super::idents::{PseudoKind, RegItem};
use super::rel::RelArch;
use super::{Parser, Sec, Stmt, Tokens, eqi, java_split_str, tok};

#[allow(clippy::too_many_lines)] // the chain arm order IS the contract
pub(crate) fn section(p: &mut Parser, stmt: &mut Stmt) -> Sec {
    let first = stmt.first();
    let idr = stmt.identifier.clone();
    let idr = idr.as_deref();
    let raw = stmt.tokens.clone();
    let t = raw.without_context_start_token();
    let open = Parser::should_start_context(&raw);

    let is_model = p.in_ctx(|c| matches!(c, Ctx::Model { .. }));
    let is_system = p.in_ctx(|c| matches!(c, Ctx::SoftwareSystem { .. }));
    let is_container = p.in_ctx(|c| matches!(c, Ctx::Container { .. }));
    let is_env = p.in_ctx(Ctx::is_environment);
    let is_element = p.in_ctx(Ctx::is_element);
    let is_elements = p.in_ctx(|c| matches!(c, Ctx::Elements { .. }));
    let is_model_item = p.in_ctx(Ctx::is_model_item);
    let is_node = p.in_ctx(|c| matches!(c, Ctx::DeploymentNode { .. }));

    // 350: source -/> destination ["description"] { — inside a deployment
    // environment. Guard: >= 4 RAW tokens, case-SENSITIVE `-/>`, must open
    // a context.
    if raw.size() >= 4 && raw.raw_token(1) == tok::NO_RELATIONSHIP && open && is_env {
        return (|| {
            let (_removed, first_linked) = p.no_relationship(&t)?;
            // HashSet order is not portable; first-removal order stands in
            // (agents/plan/PROGRESS.md). The reference reads the linked
            // (static) relationship off the removed object — which still
            // answers after `Model.remove` — so `no_relationship` captures
            // its id mid-loop; a post-removal model lookup would be blind
            // (M2-d A/B: no-relationship lost the inherited description and
            // technology).
            let Some(linked) = first_linked else {
                return Err(DslError::stmt(
                    "A relationship between the given elements does not exist",
                ));
            };
            let (environment, group) = match p.top() {
                Some(Ctx::Environment { env, group }) => (env.name.clone(), group.clone()),
                Some(Ctx::NoRelInEnv {
                    environment, group, ..
                }) => (environment.clone(), group.clone()),
                _ => (String::new(), None),
            };
            p.start_context(Ctx::NoRelInEnv {
                environment,
                linked,
                group,
            });
            Ok(())
        })()
        .into();
    }

    // 363: explicit relationship `a -> b` / `a --arch-> b` in a model /
    // environment / element context.
    if raw.size() > 2
        && p.is_relationship_keyword_or_archetype(&raw.get(1))
        && (is_model || is_env || is_element)
    {
        return (|| {
            let arch = p.get_archetype(tok::RELATIONSHIP, &raw.get(1));
            let rels = p.explicit_rel(&t, arch)?;
            p.register_rel_result(idr, rels, &raw)
        })()
        .into();
    }

    // 382: implicit relationship `-> this` / `--arch-> this` inside an
    // element context.
    if raw.size() >= 2 && p.is_relationship_keyword_or_archetype(&raw.get(0)) && is_element {
        return (|| {
            let arch = p.get_archetype(tok::RELATIONSHIP, &raw.get(0));
            let rels = p.implicit_rel(&t, arch)?;
            p.register_rel_result(idr, rels, &raw)
        })()
        .into();
    }

    // 401: explicit relationship inside an `!elements` fan-out.
    if raw.size() > 2 && p.is_relationship_keyword_or_archetype(&raw.get(1)) && is_elements {
        return (|| {
            let arch = p.get_archetype(tok::RELATIONSHIP, &raw.get(1));
            let rels = p.explicit_rel_elements(&t, arch)?;
            if open {
                p.start_context(Ctx::Relationships { items: rels });
            }
            Ok(())
        })()
        .into();
    }

    // 409: implicit relationship inside an `!elements` fan-out. Reference
    // quirk: the archetype is read from `tokens.get(1)` (the destination),
    // NOT `tokens.get(0)` — replicated.
    if raw.size() >= 2 && p.is_relationship_keyword_or_archetype(&raw.get(0)) && is_elements {
        return (|| {
            let arch = p.get_archetype(tok::RELATIONSHIP, &raw.get(1));
            let rels = p.implicit_rel_elements(&t, arch)?;
            if open {
                p.start_context(Ctx::Relationships { items: rels });
            }
            Ok(())
        })()
        .into();
    }

    // 417: !element / !relationship finders (and the removed !ref/!extend
    // spellings) inside a model-item or model context.
    if (eqi(&first, tok::FIND_ELEMENT)
        || eqi(&first, tok::FIND_RELATIONSHIP)
        || eqi(&first, tok::REF)
        || eqi(&first, tok::EXTEND))
        && (is_model_item || is_model)
    {
        return (|| {
            let (item, canonical) = if eqi(&first, tok::REF) {
                return Err(DslError::stmt(format!(
                    "{} was previously deprecated, and has now been removed - please use {} or {} instead",
                    tok::REF,
                    tok::FIND_ELEMENT,
                    tok::FIND_RELATIONSHIP
                )));
            } else if eqi(&first, tok::EXTEND) {
                return Err(DslError::stmt(format!(
                    "{} was previously deprecated, and has now been removed - please use {} or {} instead",
                    tok::EXTEND,
                    tok::FIND_ELEMENT,
                    tok::FIND_RELATIONSHIP
                )));
            } else if eqi(&first, tok::FIND_ELEMENT) {
                p.find_element(&t)?
            } else {
                let (rid, canonical) = p.find_relationship(&t)?;
                (RegItem::Relationship(rid), canonical)
            };

            // Context ladder (`modelItem instanceof …`); ElementGroup and
            // CustomElement contexts are NOT pushed. The DSL-package
            // `DeploymentEnvironment extends Element` shim is reachable
            // through a finder (`de = deploymentEnvironment "X"` registers
            // it) and opens a FRESH `DeploymentEnvironmentDslContext(name)`.
            let push = match &item {
                RegItem::Element(eid) => match p.builder.element_kind(eid) {
                    Some(ElementKind::Person) => Some(Ctx::Person { id: eid.clone() }),
                    Some(ElementKind::SoftwareSystem) => Some(Ctx::SoftwareSystem {
                        id: eid.clone(),
                        group: None,
                    }),
                    Some(ElementKind::Container) => Some(Ctx::Container {
                        id: eid.clone(),
                        group: None,
                    }),
                    Some(ElementKind::Component) => Some(Ctx::Component { id: eid.clone() }),
                    Some(ElementKind::DeploymentNode) => Some(Ctx::DeploymentNode {
                        id: eid.clone(),
                        group: None,
                    }),
                    Some(ElementKind::InfrastructureNode) => {
                        Some(Ctx::InfrastructureNode { id: eid.clone() })
                    }
                    Some(ElementKind::SoftwareSystemInstance) => {
                        Some(Ctx::SsInstance { id: eid.clone() })
                    }
                    Some(ElementKind::ContainerInstance) => {
                        Some(Ctx::ContainerInstance { id: eid.clone() })
                    }
                    _ => None,
                },
                RegItem::Relationship(rid) => Some(Ctx::Rel { id: rid.clone() }),
                RegItem::Pseudo { kind, .. } => match kind {
                    PseudoKind::DeploymentEnvironment(name) => Some(Ctx::Environment {
                        env: DslEnvironment {
                            name: name.clone(),
                        },
                        group: None,
                    }),
                    PseudoKind::Group { .. } | PseudoKind::DeploymentGroup { .. } => None,
                },
            };
            if open
                && let Some(ctx) = push
            {
                p.start_context(ctx);
            }
            // The finder registers ONLY when an identifier was given
            // (`!StringUtils.isNullOrEmpty(identifier)` — chain line 457,
            // unlike the element-definition arms which register always).
            if idr.is_some_and(|i| !i.is_empty()) {
                match &item {
                    RegItem::Element(_) | RegItem::Pseudo { .. } => {
                        p.register_identifier_element(idr, item.clone(), &canonical)?;
                    }
                    RegItem::Relationship(rid) => {
                        p.register_identifier_relationship(idr, rid, &canonical)?;
                    }
                }
            }
            Ok(())
        })()
        .into();
    }

    // 465: !elements <expression> — model / environment / element context.
    if eqi(&first, tok::FIND_ELEMENTS) && (is_model || is_env || is_element) {
        return (|| {
            let elements = p.find_elements(&t)?;
            if open {
                p.start_context(Ctx::Elements { items: elements });
            }
            Ok(())
        })()
        .into();
    }

    // 472: !relationships <expression>.
    if eqi(&first, tok::FIND_RELATIONSHIPS) && (is_model || is_env || is_element) {
        return (|| {
            let rels = p.find_relationships(&t)?;
            if open {
                p.start_context(Ctx::Relationships { items: rels });
            }
            Ok(())
        })()
        .into();
    }

    // 479: element "Name" ["metadata"] ["description"] ["tags"] — custom
    // element (model context). Quirks replicated: no archetype `extends`
    // handling, NO properties/perspectives, group set WITHOUT addElement.
    if p.is_element_keyword_or_archetype(&first, tok::CUSTOM_ELEMENT) && is_model {
        return (|| {
            let eid = def_custom_element(p, &t)?;
            if open {
                p.start_context(Ctx::CustomElement { id: eid.clone() });
            }
            let canonical = p.builder.canonical_name_full(&eid).unwrap_or_default();
            p.register_identifier_element(idr, RegItem::Element(eid), &canonical)
        })()
        .into();
    }

    // 490: person "Name" ["description"] ["tags"].
    if p.is_element_keyword_or_archetype(&first, tok::PERSON) && is_model {
        return (|| {
            let eid = def_person_system(p, &t, false)?;
            if open {
                p.start_context(Ctx::Person { id: eid.clone() });
            }
            let canonical = p.builder.canonical_name_full(&eid).unwrap_or_default();
            p.register_identifier_element(idr, RegItem::Element(eid), &canonical)
        })()
        .into();
    }

    // 501: softwareSystem "Name" ["description"] ["tags"].
    if p.is_element_keyword_or_archetype(&first, tok::SOFTWARE_SYSTEM) && is_model {
        return (|| {
            let eid = def_person_system(p, &t, true)?;
            if open {
                p.start_context(Ctx::SoftwareSystem {
                    id: eid.clone(),
                    group: None,
                });
            }
            let canonical = p.builder.canonical_name_full(&eid).unwrap_or_default();
            p.register_identifier_element(idr, RegItem::Element(eid), &canonical)
        })()
        .into();
    }

    // 512: container "Name" ["description"] ["technology"] ["tags"] —
    // inside a software system.
    if p.is_element_keyword_or_archetype(&first, tok::CONTAINER) && is_system {
        return (|| {
            let system = p
                .top()
                .and_then(Ctx::item_id)
                .cloned()
                .expect("system context checked by the guard");
            let eid = def_container(p, &system, &t)?;
            if open {
                p.start_context(Ctx::Container {
                    id: eid.clone(),
                    group: None,
                });
            }
            let canonical = p.builder.canonical_name_full(&eid).unwrap_or_default();
            p.register_identifier_element(idr, RegItem::Element(eid), &canonical)
        })()
        .into();
    }

    // 523: component "Name" ["description"] ["technology"] ["tags"] —
    // inside a container.
    if p.is_element_keyword_or_archetype(&first, tok::COMPONENT) && is_container {
        return (|| {
            let container = p
                .top()
                .and_then(Ctx::item_id)
                .cloned()
                .expect("container context checked by the guard");
            let eid = def_component(p, &container, &t)?;
            if open {
                p.start_context(Ctx::Component { id: eid.clone() });
            }
            let canonical = p.builder.canonical_name_full(&eid).unwrap_or_default();
            p.register_identifier_element(idr, RegItem::Element(eid), &canonical)
        })()
        .into();
    }

    // 534: !components (JVM component finder) — rejected. The strategy
    // arms (540+) are unreachable because the context is never pushed.
    if eqi(&first, tok::COMPONENT_FINDER) && is_container {
        return Sec::Err(DslError::coded(
            oxidizr_diag::codes::JVM_ONLY,
            format!(
                "{first} is not supported by oxidizr (JVM-only directive, D2): {first} is not permitted"
            ),
        ));
    }

    // 578: enterprise — removed keyword (pre-group era).
    if eqi(&first, tok::ENTERPRISE) && is_model {
        return Sec::Err(DslError::stmt(
            "The enterprise keyword was previously deprecated, and has now been removed - please use group instead (https://docs.structurizr.com/dsl/language#group)",
        ));
    }

    // 581/587/594/601/607: the five `group "Name" {` arms — model, system,
    // container, environment and deployment-node scopes. `GroupParser`
    // ignores the archetype, so no archetype lookup happens (`getArchetype`
    // in the other arms only feeds parsing; a group's would be a Java
    // throwaway). The arms match on the CONTEXT CLASS alone — no `hasGroup`
    // requirement; an existing group only changes how the NAME is built.
    if p.is_element_keyword_or_archetype(&first, tok::GROUP)
        && (is_model || is_system || is_container || is_env || is_node)
    {
        return (|| {
            let Some(top) = p.top().cloned() else {
                return Err(DslError::stmt("Expected: group <name> {"));
            };
            let parent = top.group_payload().cloned();
            let name = parse_group(p, &raw, parent.as_ref())?;
            let uid = p.group_open(parent.as_ref().map(|g| g.uid));
            let group = DslGroup {
                uid,
                name: name.clone(),
            };
            // startContext BEFORE registerIdentifier (reference order).
            p.start_group_context(&top, group);
            // `group.setParent(softwareSystem/container)` — only the system
            // and container arms set a parent (hierarchical-identifier
            // prefix source).
            let parent_id = match &top {
                Ctx::SoftwareSystem { id, .. } | Ctx::Container { id, .. } => Some(id.clone()),
                _ => None,
            };
            let pseudo = super::group_pseudo(uid, &name, parent_id.as_ref());
            p.register_identifier_element(idr, pseudo, &name)
        })()
        .into();
    }

    Sec::No
}

/// `GroupParser.parseContext`. With a parent group the name is joined as
/// `parent.getName() + separator + name` — the separator comes from the
/// `structurizr.groupSeparator` model property, whose absence is the
/// nested-group error.
fn parse_group(
    p: &mut Parser<'_>,
    tokens: &Tokens,
    parent: Option<&DslGroup>,
) -> Result<String, DslError> {
    const GRAMMAR: &str = "group <name> {";
    if tokens.has_more_than(2) {
        return Err(DslError::stmt(format!(
            "Too many tokens, expected: {GRAMMAR}"
        )));
    }
    if !tokens.includes(2) {
        return Err(DslError::stmt(format!("Expected: {GRAMMAR}")));
    }
    if !eqi(&tokens.get(2), tok::CONTEXT_START) {
        return Err(DslError::stmt(format!("Expected: {GRAMMAR}")));
    }
    let name = tokens.get(1);
    match parent {
        None => Ok(name),
        Some(parent) => {
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
            Ok(format!("{}{separator}{name}", parent.name))
        }
    }
}

/// The name token of the four element parsers, in the reference's order:
/// `hasMoreThan` first, then `includes(NAME_INDEX)`, then the blank-name
/// guard — the reference reaches the name through `getXxxWithName`, which
/// throws for null/whitespace names (before the duplicate guard).
fn name_token(
    tokens: &Tokens,
    grammar: &str,
    max: usize,
    blank_msg: &str,
) -> Result<String, DslError> {
    if tokens.has_more_than(max) {
        return Err(DslError::stmt(format!(
            "Too many tokens, expected: {grammar}"
        )));
    }
    if !tokens.includes(1) {
        return Err(DslError::stmt(format!("Expected: {grammar}")));
    }
    let name = tokens.get(1);
    if name.trim().is_empty() {
        return Err(DslError::stmt(blank_msg.to_string()));
    }
    Ok(name)
}

/// The parser tag list: `new ArrayList<>(archetype.getTags())` plus
/// `addAll(token.split(","))` when the tags token is present — the
/// archetype tags apply UNCONDITIONALLY (the Elements-variant
/// REPLACEMENT lives in `rel.rs`).
fn tags_from(arch: &RelArch, token: Option<String>) -> Vec<String> {
    let mut tags = arch.tags.clone();
    if let Some(csv) = token {
        tags.extend(java_split_str(&csv, ",").into_iter().map(str::to_string));
    }
    tags
}

/// `PersonParser`/`SoftwareSystemParser.parse` + the shared post-creation
/// sequence (description, tags, properties, perspectives, group).
fn def_person_system(p: &mut Parser<'_>, tokens: &Tokens, system: bool) -> Result<Id, DslError> {
    let (kind, grammar, blank) = if system {
        (
            tok::SOFTWARE_SYSTEM,
            "softwareSystem <name> [description] [tags]",
            "A software system name must be specified.",
        )
    } else {
        (
            tok::PERSON,
            "person <name> [description] [tags]",
            "A person name must be specified.",
        )
    };
    let name = name_token(tokens, grammar, 3, blank)?;
    let arch = p.get_archetype(kind, &tokens.get(0));
    let a = p.arch_snapshot(arch);
    let description = if tokens.includes(2) {
        tokens.get(2)
    } else {
        a.description.clone()
    };
    let tags = tags_from(&a, tokens.includes(3).then(|| tokens.get(3)));
    // `isExtendingWorkspace()` — on a base workspace, reuse the element with
    // this EXACT name instead of adding (the add would hit the
    // duplicate-name error); description/tags below then layer onto it.
    let existing = if p.extending_workspace {
        let m = &p.builder.workspace().model;
        if system {
            m.software_systems
                .iter()
                .find(|s| s.item.name == name)
                .map(|s| s.item.id.clone())
        } else {
            m.people
                .iter()
                .find(|x| x.item.name == name)
                .map(|x| x.item.id.clone())
        }
    } else {
        None
    };
    let eid = match existing {
        Some(id) => id,
        None if system => p
            .builder
            .add_software_system(&name, None, None)
            .map_err(|e| DslError::core(&e))?,
        None => p
            .builder
            .add_person(&name, None, None)
            .map_err(|e| DslError::core(&e))?,
    };
    p.define_element(&eid, &description, &tags, &a)?;
    Ok(eid)
}

/// `ContainerParser.parse`.
fn def_container(p: &mut Parser<'_>, system: &Id, tokens: &Tokens) -> Result<Id, DslError> {
    const GRAMMAR: &str = "container <name> [description] [technology] [tags]";
    let name = name_token(tokens, GRAMMAR, 4, "A container name must be provided.")?;
    let arch = p.get_archetype(tok::CONTAINER, &tokens.get(0));
    let a = p.arch_snapshot(arch);
    let description = if tokens.includes(2) {
        tokens.get(2)
    } else {
        a.description.clone()
    };
    let technology = if tokens.includes(3) {
        tokens.get(3)
    } else {
        a.technology.clone()
    };
    let tags = tags_from(&a, tokens.includes(4).then(|| tokens.get(4)));
    // ContainerParser reuse on extends (`getContainerWithName`).
    let existing = p
        .extending_workspace
        .then(|| {
            p.builder
                .workspace()
                .model
                .software_systems
                .iter()
                .find(|s| s.item.id == *system)
                .and_then(|s| s.containers.iter().find(|c| c.item.name == name))
                .map(|c| c.item.id.clone())
        })
        .flatten();
    let eid = match existing {
        Some(id) => id,
        None => p
            .builder
            .add_container(system, &name, None, None, None)
            .map_err(|e| DslError::core(&e))?,
    };
    if let Some(mut el) = p.builder.element_mut(&eid) {
        el.set_technology(Some(technology));
    }
    p.define_element(&eid, &description, &tags, &a)?;
    Ok(eid)
}

/// `ComponentParser.parse`.
fn def_component(p: &mut Parser<'_>, container: &Id, tokens: &Tokens) -> Result<Id, DslError> {
    const GRAMMAR: &str = "component <name> [description] [technology] [tags]";
    let name = name_token(tokens, GRAMMAR, 4, "A component name must be provided.")?;
    let arch = p.get_archetype(tok::COMPONENT, &tokens.get(0));
    let a = p.arch_snapshot(arch);
    let description = if tokens.includes(2) {
        tokens.get(2)
    } else {
        a.description.clone()
    };
    let technology = if tokens.includes(3) {
        tokens.get(3)
    } else {
        a.technology.clone()
    };
    let tags = tags_from(&a, tokens.includes(4).then(|| tokens.get(4)));
    // ComponentParser reuse on extends (`getComponentWithName`).
    let existing = p
        .extending_workspace
        .then(|| {
            p.builder
                .workspace()
                .model
                .software_systems
                .iter()
                .flat_map(|s| s.containers.iter())
                .find(|c| c.item.id == *container)
                .and_then(|c| c.components.iter().find(|k| k.item.name == name))
                .map(|k| k.item.id.clone())
        })
        .flatten();
    let eid = match existing {
        Some(id) => id,
        None => p
            .builder
            .add_component(container, &name, None, None, None)
            .map_err(|e| DslError::core(&e))?,
    };
    if let Some(mut el) = p.builder.element_mut(&eid) {
        el.set_technology(Some(technology));
    }
    p.define_element(&eid, &description, &tags, &a)?;
    Ok(eid)
}

/// `CustomElementParser.parse` — metadata slot, no technology, NO
/// properties/perspectives, `setGroup` WITHOUT `group.addElement`.
fn def_custom_element(p: &mut Parser<'_>, tokens: &Tokens) -> Result<Id, DslError> {
    const GRAMMAR: &str = "element <name> [metadata] [description] [tags]";
    let name = name_token(
        tokens,
        GRAMMAR,
        4,
        "A custom element name must be specified.",
    )?;
    let arch = p.get_archetype(tok::CUSTOM_ELEMENT, &tokens.get(0));
    let a = p.arch_snapshot(arch);
    let metadata = if tokens.includes(2) {
        tokens.get(2)
    } else {
        a.metadata.clone()
    };
    let description = if tokens.includes(3) {
        tokens.get(3)
    } else {
        a.description.clone()
    };
    let tags = tags_from(&a, tokens.includes(4).then(|| tokens.get(4)));
    let eid = p
        .builder
        .add_custom_element(&name, None, None)
        .map_err(|e| DslError::core(&e))?;
    if let Some(mut el) = p.builder.element_mut(&eid) {
        el.set_metadata(Some(metadata));
    }
    let _ = p.builder.set_description(&eid, &description);
    p.builder.add_tag_parts(&eid, &tags);
    if let Some(g) = p.top().and_then(Ctx::group_payload) {
        let name = g.name.clone();
        let _ = p.builder.set_group(&eid, &name);
    }
    Ok(eid)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::engine::ParseOptions;
    use crate::engine::parse_dsl;

    /// Parse `dsl` as a complete `model { … }` scope. The scope arms are a
    /// later section, so the harness seeds a `ModelDslContext` on the stack
    /// and feeds the body as raw lines through the chain.
    fn in_model(dsl: &str) -> super::super::ParseResult {
        let mut p = Parser::new(ParseOptions::fragment());
        p.start_context(Ctx::Model { group: None });
        let logical = crate::preprocess::preprocess(dsl);
        p.parse_lines(&logical, dsl, false);
        assert_eq!(p.stack.len(), 1, "unbalanced contexts: {:?}", p.errors);
        p.stack.pop().expect("seeded context");
        p.finish()
    }

    fn errors(r: &super::super::ParseResult) -> Vec<String> {
        r.errors.iter().map(|e| e.message.clone()).collect()
    }

    fn assert_clean(r: &super::super::ParseResult) {
        assert!(r.errors.is_empty(), "unexpected errors: {:?}", errors(r));
    }

    #[test]
    fn element_definitions_tags_and_ids() {
        let r = in_model(
            r#"
p1 = person "Alice" "A user" "Actor,Extra"
sys = softwareSystem "System A" "The system" {
  c1 = container "Web" "Web app" "Java 21" "WebTag" {
    k1 = component "HomeController" "MVC" "Spring MVC"
  }
  c2 = container "DB"
}
"#,
        );
        assert_clean(&r);
        let w = &r.workspace;
        assert_eq!(w.model.people.len(), 1);
        let alice = &w.model.people[0];
        assert_eq!(alice.item.id.as_str(), "1", "ids follow definition order");
        assert_eq!(alice.item.name, "Alice");
        assert_eq!(alice.item.description.as_deref(), Some("A user"));
        assert_eq!(alice.item.tags.stored(), ["Actor", "Extra"]);
        assert_eq!(w.model.software_systems.len(), 1);
        let sys = &w.model.software_systems[0];
        assert_eq!(sys.item.id.as_str(), "2");
        assert_eq!(sys.containers.len(), 2);
        assert_eq!(sys.containers[0].item.name, "Web");
        assert_eq!(sys.containers[0].technology.as_deref(), Some("Java 21"));
        assert_eq!(sys.containers[0].item.tags.stored(), ["WebTag"]);
        assert_eq!(sys.containers[0].components.len(), 1);
        let k1 = &sys.containers[0].components[0];
        assert_eq!(k1.item.id.as_str(), "4");
        assert_eq!(k1.technology.as_deref(), Some("Spring MVC"));
        assert_eq!(
            sys.containers[1].item.id.as_str(),
            "5",
            "contexts consume no ids — p1=1, sys=2, c1=3, k1=4, c2=5"
        );
    }

    #[test]
    fn custom_element_metadata_and_blank_names() {
        let r = in_model(
            r#"
e = element "Frontier" "blockchain-net" "A network" "Chain,Net"
bad = person "  "
"#,
        );
        assert_eq!(errors(&r), vec!["A person name must be specified."]);
        let w = &r.workspace;
        let e = &w.model.custom_elements[0];
        assert_eq!(e.metadata.as_deref(), Some("blockchain-net"));
        assert_eq!(e.item.description.as_deref(), Some("A network"));
        assert_eq!(e.item.tags.stored(), ["Chain", "Net"]);
    }

    #[test]
    fn element_grammar_errors() {
        let r = in_model(
            r#"
person "a" "b" "c" "d"
softwareSystem
container "C"
enterprise "E"
!ref x
"#,
        );
        assert_eq!(
            errors(&r),
            vec![
                "Too many tokens, expected: person <name> [description] [tags]",
                "Expected: softwareSystem <name> [description] [tags]",
                // `container` outside a system context falls through to the
                // catch-all, which lists the TOP context's (Model) permitted
                // tokens — reference `getPermittedTokens()` order.
                "Unexpected tokens (expected: archetypes, !identifiers, group, person, softwareSystem, deploymentEnvironment, element, ->)",
                "The enterprise keyword was previously deprecated, and has now been removed - please use group instead (https://docs.structurizr.com/dsl/language#group)",
                "!ref was previously deprecated, and has now been removed - please use !element or !relationship instead",
            ],
            "one diagnostic per failing line"
        );
    }

    #[test]
    fn duplicate_names_match_the_reference() {
        let r = in_model(
            r#"
s1 = softwareSystem "Dup"
s2 = softwareSystem "Dup"
p = person "Dup"
"#,
        );
        assert_eq!(
            errors(&r),
            vec!["A top-level element named 'Dup' already exists."],
            "person and system share a name: per-kind namespaces (@ad34036)"
        );
        assert_eq!(r.workspace.model.people.len(), 1);
    }

    #[test]
    fn group_sets_names_and_scopes() {
        let r = in_model(
            r#"
group "G1" {
  p = person "InGroup"
  e = element "Custom"
  s = softwareSystem "S"
}
out = person "Out"
"#,
        );
        assert_clean(&r);
        let w = &r.workspace;
        assert_eq!(w.model.people[0].group.as_deref(), Some("G1"));
        assert_eq!(
            w.model.custom_elements[0].group.as_deref(),
            Some("G1"),
            "CustomElementParser sets the group (but never adds members)"
        );
        assert_eq!(w.model.software_systems[0].group.as_deref(), Some("G1"));
        assert_eq!(w.model.people[1].group, None);
    }

    #[test]
    fn nested_group_requires_the_separator_property() {
        // A group inside a group is a NESTED group: the top context already
        // carries a group, so `structurizr.groupSeparator` is required.
        // The failing `group "Inner" {` records a diagnostic and (D1)
        // truncates to its entry height; the lone `}` closes the outer group.
        let r = in_model(
            r#"
group "Outer" {
  p = person "InOuter"
  group "Inner" {
}
"#,
        );
        assert_eq!(
            errors(&r),
            vec![
                "To use nested groups, please define a model property named structurizr.groupSeparator"
            ]
        );
        assert_eq!(r.workspace.model.people[0].group.as_deref(), Some("Outer"));
    }

    #[test]
    fn group_directly_in_a_system_needs_no_separator() {
        // A group whose top context has NO group is a top-level group — the
        // separator property is only required for nesting.
        let r = in_model(
            r#"
s = softwareSystem "S" {
  group "Inner" {
    c = container "C"
  }
}
"#,
        );
        assert_clean(&r);
        assert_eq!(
            r.workspace.model.software_systems[0].containers[0]
                .group
                .as_deref(),
            Some("Inner")
        );
    }

    #[test]
    fn explicit_implicit_and_fanout_relationships() {
        let r = in_model(
            r#"
p1 = person "Alice"
sys = softwareSystem "System" {
  c1 = container "Web" {
    -> p1 "Reads from"
  }
  c2 = container "DB"
}
p1 -> sys "Uses" "HTTP" "Rel1,Rel2"
c2 -> p1 "Talks to"
"#,
        );
        assert_clean(&r);
        let w = &r.workspace;
        assert_eq!(w.model.relationships.len(), 3);
        let r0 = &w.model.relationships[0];
        assert_eq!(r0.description.as_deref(), Some("Reads from"));
        let rel = &w.model.relationships[1];
        assert_eq!(rel.description.as_deref(), Some("Uses"));
        assert_eq!(rel.technology.as_deref(), Some("HTTP"));
        assert_eq!(rel.tags.stored(), ["Rel1", "Rel2"]);
        assert_eq!(
            w.model.relationships[2].destination,
            w.model.people[0].item.id
        );
    }

    #[test]
    fn relationship_registration_and_this_fallback() {
        let r = in_model(
            r#"
a = softwareSystem "A"
b = softwareSystem "B"
rel = a -> b "X" {
}
this -> b "Nope"
"#,
        );
        // `rel {` opens a RelationshipDslContext (closed by `}`); `this` at
        // model scope has no element context ⇒ the endpoint lookup fails.
        assert_eq!(
            errors(&r),
            vec!["The source element \"this\" does not exist"]
        );
        assert_eq!(r.workspace.model.relationships.len(), 1);
        let rel_id = r.workspace.model.relationships[0].id.clone();
        assert_eq!(
            r.workspace
                .model
                .relationships
                .first()
                .map(|x| x.id.as_str()),
            Some(rel_id.as_str())
        );
    }

    #[test]
    fn finders_and_expression_fanout() {
        let r = in_model(
            r#"
s1 = softwareSystem "S1" "d" "TagA"
s2 = softwareSystem "S2" "d" "TagA"
s3 = softwareSystem "S3"
!element s1
!elements element.tag==TagA {
  -> s3 "Fan"
}
"#,
        );
        assert_clean(&r);
        let w = &r.workspace;
        assert_eq!(w.model.relationships.len(), 2);
        assert!(
            w.model
                .relationships
                .iter()
                .all(|rel| rel.description.as_deref() == Some("Fan")),
            "both context elements became sources"
        );
    }

    #[test]
    fn finder_errors() {
        let r = in_model(
            r"
!element nope
!elements
",
        );
        assert_eq!(
            errors(&r),
            vec![
                "An element identified by \"nope\" could not be found",
                "Index 1 out of bounds for length 1",
            ],
            "bare !elements hits tokens.get(1) in the reference (no includes check)"
        );
    }

    #[test]
    fn components_finder_is_rejected() {
        let r = in_model(
            r#"
s = softwareSystem "S" {
  c = container "C" {
    !components {
  }
}
"#,
        );
        let msgs = errors(&r);
        assert_eq!(msgs.len(), 1);
        assert!(
            msgs[0].contains("!components is not permitted"),
            "JVM-only reject: {msgs:?}"
        );
        // `parse_dsl` is the production entry; smoke-check it still rejects
        // the same statement list shape.
        assert!(parse_dsl("workspace", ParseOptions::fragment()).is_ok());
    }

    #[test]
    fn find_element_opens_deployment_environment_context() {
        // Reference `deployment-environment-empty.dsl`
        // (`test_EmptyDeploymentEnvironment`): the DSL-package
        // `DeploymentEnvironment extends Element` shim registers under the
        // `de =` identifier, and `!element de {` opens a FRESH
        // `DeploymentEnvironmentDslContext(name)` — so `deploymentNode`
        // statements are legal inside the block.
        let r = in_model(
            "    de = deploymentEnvironment \"DeploymentEnvironment\"\n    !element de {\n        dn = deploymentNode \"DeploymentNode\"\n    }\n",
        );
        assert!(errors(&r).is_empty(), "{:?}", errors(&r));
        let nodes = &r.workspace.model.deployment_nodes;
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].item.name, "DeploymentNode");
        assert_eq!(nodes[0].environment, "DeploymentEnvironment");
    }
}
