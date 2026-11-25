//! Include/exclude arms of the view sections: the static, deployment and
//! custom content add/remove legs with the official wildcard matching and
//! the "not permitted" catch. The view-definition, animation, step and
//! configuration arms live in the sibling modules; the shared free helpers
//! stay in the parent module.

use oxidizr_core::validate::ElementKind;
use oxidizr_core::{CoreError, Id, ViewRef};

use super::super::expr::{ExprScope, Mi, is_expression};
use super::{DslError, Parser, Tokens};
use super::{expected, is_trim_empty};

// ---------------------------------------------------------------------------
// view content — custom, static, deployment
// ---------------------------------------------------------------------------

const WILDCARD: &str = "*";
const WILDCARD_RELUCTANT: &str = "*?";
const ELEMENT_WILDCARD: &str = "element==*";

/// the model-view-content parser's add-element-to-view check ladders: `Ok(false)` from the
/// core add is the swallowed the element-not-permitted rejection.
fn add_element_ignoring_not_permitted(
    p: &mut Parser,
    view: ViewRef,
    id: &Id,
) -> Result<(), DslError> {
    match p.builder.view_add_element(view, id, true) {
        Ok(()) | Err(CoreError::ElementNotPermittedInView(_)) => Ok(()),
        Err(e) => Err(DslError::core(&e)),
    }
}

/// The deployment-view sibling of [`add_element_ignoring_not_permitted`]:
/// dispatches to the kind-matched `DeploymentView.add` overload (ancestor
/// chain incl.) inside the same per-item
/// `catch (the element-not-permitted rejection) { // ignore }`.
fn add_deployment_element_ignoring_not_permitted(
    p: &mut Parser,
    view: ViewRef,
    id: &Id,
) -> Result<(), DslError> {
    match p.builder.view_add_deployment_element(view, id, true) {
        Ok(()) | Err(CoreError::ElementNotPermittedInView(_)) => Ok(()),
        Err(e) => Err(DslError::core(&e)),
    }
}

/// The blank check on `identifier` — the add-ladder error fires on
/// the identifier path only (expressions pass `null`).
fn cannot_add(identifier: Option<&str>) -> Option<DslError> {
    identifier.filter(|i| !is_trim_empty(i)).map(|i| {
        DslError::stmt(format!(
            "The element \"{i}\" can not be added to this type of view"
        ))
    })
}

pub(super) fn static_include(
    p: &mut Parser,
    view: ViewRef,
    tokens: &Tokens,
) -> Result<(), DslError> {
    if !tokens.includes(1) {
        // The two grammar strings depend on the view class (context/
        // container/component have the reluctant `*?`).
        return Err(expected(
            if matches!(
                view,
                ViewRef::Context(_) | ViewRef::Container(_) | ViewRef::Component(_)
            ) {
                "include <*|*?|identifier|expression> [*|identifier|expression...]"
            } else {
                "include <*|identifier|expression> [*|identifier|expression...]"
            },
        ));
    }
    for i in 1..tokens.size() {
        let token = tokens.get(i);
        if token == WILDCARD || token == ELEMENT_WILDCARD {
            let _ = p.builder.view_add_default_elements(view, true);
        } else if token == WILDCARD_RELUCTANT {
            let _ = p.builder.view_add_default_elements(view, false);
        } else if is_expression(&token) {
            let items = p.parse_expression(&token, ExprScope::Static)?;
            for item in items {
                add_static_item(p, view, item, None)?;
            }
        } else {
            let items = p.parse_identifier(&token, ExprScope::Static)?;
            for item in items {
                add_static_item(p, view, item, Some(&*token))?;
            }
        }
    }
    Ok(())
}

fn add_static_item(
    p: &mut Parser,
    view: ViewRef,
    item: Mi,
    identifier: Option<&str>,
) -> Result<(), DslError> {
    match item {
        Mi::Element(id) => {
            let permitted = match p.builder.element_kind(&id) {
                Some(
                    ElementKind::CustomElement | ElementKind::Person | ElementKind::SoftwareSystem,
                ) => true,
                Some(ElementKind::Container) => {
                    matches!(view, ViewRef::Container(_) | ViewRef::Component(_))
                }
                Some(ElementKind::Component) => matches!(view, ViewRef::Component(_)),
                _ => false,
            };
            if permitted {
                add_element_ignoring_not_permitted(p, view, &id)?;
            } else if let Some(e) = cannot_add(identifier) {
                return Err(e);
            }
        }
        Mi::Rel(id) => {
            // `addRelationshipToView` — silent unless both ends are in view.
            let _ = p.builder.view_add_relationship(view, &id);
        }
    }
    Ok(())
}

pub(super) fn static_exclude(
    p: &mut Parser,
    view: ViewRef,
    tokens: &Tokens,
) -> Result<(), DslError> {
    if !tokens.includes(1) {
        return Err(expected(
            "exclude <identifier|expression> [identifier|expression...]",
        ));
    }
    for i in 1..tokens.size() {
        let token = tokens.get(i);
        let items = if is_expression(&token) {
            p.parse_expression(&token, ExprScope::Static)?
        } else {
            p.parse_identifier(&token, ExprScope::Static)?
        };
        for item in items {
            remove_static_item(p, view, item);
        }
    }
    Ok(())
}

fn remove_static_item(p: &mut Parser, view: ViewRef, item: Mi) {
    match item {
        Mi::Element(id) => {
            let permitted = match p.builder.element_kind(&id) {
                Some(
                    ElementKind::CustomElement | ElementKind::Person | ElementKind::SoftwareSystem,
                ) => true,
                Some(ElementKind::Container) => {
                    matches!(view, ViewRef::Container(_) | ViewRef::Component(_))
                }
                Some(ElementKind::Component) => matches!(view, ViewRef::Component(_)),
                _ => false,
            };
            if permitted {
                let _ = p.builder.view_remove_element(view, &id);
            }
        }
        Mi::Rel(id) => p.builder.view_remove_relationship(view, &id),
    }
}

pub(super) fn deployment_include(
    p: &mut Parser,
    view: ViewRef,
    tokens: &Tokens,
) -> Result<(), DslError> {
    if !tokens.includes(1) {
        return Err(expected(
            "include <*|identifier|expression> [*|identifier|expression...]",
        ));
    }
    for i in 1..tokens.size() {
        let token = tokens.get(i);
        if token == WILDCARD || token == ELEMENT_WILDCARD {
            let _ = p.builder.view_add_default_elements(view, true);
        } else if is_expression(&token) {
            let items = p.parse_expression(&token, ExprScope::Deployment)?;
            for item in items {
                add_deployment_item(p, view, item, None)?;
            }
        } else {
            let items = p.parse_identifier(&token, ExprScope::Deployment)?;
            for item in items {
                add_deployment_item(p, view, item, Some(&*token))?;
            }
        }
    }
    Ok(())
}

fn deployment_environment(p: &Parser, view: ViewRef) -> String {
    let ViewRef::Deployment(i) = view else {
        return String::new();
    };
    p.builder
        .workspace()
        .views
        .deployment_views
        .get(i)
        .map_or(String::new(), |v| v.environment.clone())
}

fn add_deployment_item(
    p: &mut Parser,
    view: ViewRef,
    item: Mi,
    identifier: Option<&str>,
) -> Result<(), DslError> {
    match item {
        Mi::Element(id) => {
            match p.builder.element_kind(&id) {
                Some(
                    ElementKind::CustomElement
                    | ElementKind::DeploymentNode
                    | ElementKind::InfrastructureNode
                    | ElementKind::SoftwareSystemInstance
                    | ElementKind::ContainerInstance,
                ) => {
                    // The kind-matched `DeploymentView.add` overload — incl.
                    // the parent deployment-node chain walk (cross-checked).
                    add_deployment_element_ignoring_not_permitted(p, view, &id)?;
                }
                Some(ElementKind::SoftwareSystem | ElementKind::Container) => {
                    // Instances of the static element in the view's
                    // environment. Officially, the `forEach(view::add)` sits
                    // INSIDE the per-item catch — the first rejection aborts
                    // the rest of the loop.
                    let env = deployment_environment(p, view);
                    let instances = p.builder.find_instances_of(&id, &env);
                    for instance in instances {
                        match p.builder.view_add_deployment_element(view, &instance, true) {
                            Ok(()) => {}
                            Err(CoreError::ElementNotPermittedInView(_)) => break,
                            Err(e) => return Err(DslError::core(&e)),
                        }
                    }
                }
                _ => {
                    if let Some(e) = cannot_add(identifier) {
                        return Err(e);
                    }
                }
            }
        }
        Mi::Rel(id) => {
            // `addRelationshipToView` — replicated-version fallback.
            let added = p.builder.view_add_relationship(view, &id);
            if matches!(added, Ok(false)) {
                let ends_static = |rel: &Id| {
                    let w = p.builder.workspace();
                    w.model
                        .relationships
                        .iter()
                        .find(|r| &r.id == rel)
                        .is_some_and(|r| {
                            is_static_struct(p, &r.source) && is_static_struct(p, &r.destination)
                        })
                };
                if ends_static(&id) {
                    let replicated: Vec<Id> = p
                        .builder
                        .workspace()
                        .model
                        .relationships
                        .iter()
                        .filter(|r| r.linked_relationship_id.as_ref() == Some(&id))
                        .map(|r| r.id.clone())
                        .collect();
                    for r in replicated {
                        let _ = p.builder.view_add_relationship(view, &r);
                    }
                }
            } else if let Err(e) = added {
                return Err(DslError::core(&e));
            }
        }
    }
    Ok(())
}

fn is_static_struct(p: &Parser, id: &Id) -> bool {
    matches!(
        p.builder.element_kind(id),
        Some(ElementKind::SoftwareSystem | ElementKind::Container)
    )
}

pub(super) fn deployment_exclude(
    p: &mut Parser,
    view: ViewRef,
    tokens: &Tokens,
) -> Result<(), DslError> {
    if !tokens.includes(1) {
        return Err(expected(
            "exclude <identifier|expression> [identifier|expression...]",
        ));
    }
    for i in 1..tokens.size() {
        let token = tokens.get(i);
        let items = if is_expression(&token) {
            p.parse_expression(&token, ExprScope::Deployment)?
        } else {
            p.parse_identifier(&token, ExprScope::Deployment)?
        };
        for item in items {
            remove_deployment_item(p, view, item);
        }
    }
    Ok(())
}

fn remove_deployment_item(p: &mut Parser, view: ViewRef, item: Mi) {
    match item {
        Mi::Element(id) => match p.builder.element_kind(&id) {
            Some(
                ElementKind::CustomElement
                | ElementKind::DeploymentNode
                | ElementKind::InfrastructureNode
                | ElementKind::SoftwareSystemInstance
                | ElementKind::ContainerInstance,
            ) => {
                let _ = p.builder.view_remove_element(view, &id);
            }
            Some(ElementKind::SoftwareSystem | ElementKind::Container) => {
                let env = deployment_environment(p, view);
                let instances = p.builder.find_instances_of(&id, &env);
                for instance in instances {
                    let _ = p.builder.view_remove_element(view, &instance);
                }
            }
            _ => {}
        },
        Mi::Rel(id) => {
            // `removeRelationshipFromView` — plus the replicated cascade
            // over the relationship views CURRENTLY in the view.
            p.builder.view_remove_relationship(view, &id);
            let replicated: Vec<Id> = p
                .builder
                .workspace()
                .model
                .relationships
                .iter()
                .filter(|r| {
                    r.linked_relationship_id.as_ref() == Some(&id)
                        && p.builder.view_contains_relationship(view, &r.id)
                })
                .map(|r| r.id.clone())
                .collect();
            for r in replicated {
                p.builder.view_remove_relationship(view, &r);
            }
        }
    }
}

pub(super) fn custom_include(
    p: &mut Parser,
    view: ViewRef,
    tokens: &Tokens,
) -> Result<(), DslError> {
    if !tokens.includes(1) {
        return Err(expected("include <*|identifier> [*|identifier...]"));
    }
    for i in 1..tokens.size() {
        let token = tokens.get(i);
        if token == WILDCARD || token == ELEMENT_WILDCARD {
            let _ = p.builder.view_add_default_elements(view, true);
        } else if is_expression(&token) {
            let items = p.parse_expression(&token, ExprScope::Custom)?;
            for item in items {
                add_custom_item(p, view, item, None)?;
            }
        } else {
            let items = p.parse_identifier(&token, ExprScope::Custom)?;
            for item in items {
                add_custom_item(p, view, item, Some(&*token))?;
            }
        }
    }
    Ok(())
}

fn add_custom_item(
    p: &mut Parser,
    view: ViewRef,
    item: Mi,
    identifier: Option<&str>,
) -> Result<(), DslError> {
    match item {
        Mi::Element(id) => {
            if p.builder.element_kind(&id) == Some(ElementKind::CustomElement) {
                add_element_ignoring_not_permitted(p, view, &id)?;
            } else if let Some(e) = cannot_add(identifier) {
                return Err(e);
            }
        }
        Mi::Rel(id) => {
            let _ = p.builder.view_add_relationship(view, &id);
        }
    }
    Ok(())
}

pub(super) fn custom_exclude(
    p: &mut Parser,
    view: ViewRef,
    tokens: &Tokens,
) -> Result<(), DslError> {
    if !tokens.includes(1) {
        return Err(expected("exclude <identifier> [identifier...]"));
    }
    for i in 1..tokens.size() {
        let token = tokens.get(i);
        let items = if is_expression(&token) {
            p.parse_expression(&token, ExprScope::Custom)?
        } else {
            p.parse_identifier(&token, ExprScope::Custom)?
        };
        for item in items {
            if let Mi::Element(id) = item
                && p.builder.element_kind(&id) == Some(ElementKind::CustomElement)
            {
                let _ = p.builder.view_remove_element(view, &id);
            }
        }
    }
    Ok(())
}
