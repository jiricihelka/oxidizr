//! Animation-step arms of the view sections: the static, deployment and
//! custom `animation` step parsers. The definition, content and dynamic-step
//! arms live in the sibling modules; the shared free helpers stay in the
//! parent module.

use oxidizr_core::validate::ElementKind;
use oxidizr_core::{Id, ViewRef};

use super::super::expr::{ExprScope, Mi, is_expression};
use super::{DslError, Parser, Tokens};

// ---------------------------------------------------------------------------
// animation steps
// ---------------------------------------------------------------------------

pub(super) fn static_animation_step(
    p: &mut Parser,
    view: ViewRef,
    tokens: &Tokens,
    start: usize,
) -> Result<(), DslError> {
    let mut ids: Vec<Id> = Vec::new();
    for i in start..tokens.size() {
        let token = tokens.get(i);
        let items = if is_expression(&token) {
            p.parse_expression(&token, ExprScope::Static)?
        } else {
            p.parse_identifier(&token, ExprScope::Static)?
        };
        for item in items {
            if let Mi::Element(id) = item
                && matches!(
                    p.builder.element_kind(&id),
                    Some(
                        ElementKind::Person
                            | ElementKind::SoftwareSystem
                            | ElementKind::Container
                            | ElementKind::Component
                    )
                )
            {
                ids.push(id);
            }
        }
    }
    if ids.is_empty() {
        return Err(DslError::stmt("No elements were found"));
    }
    p.builder
        .view_add_animation_step(view, &ids)
        .map_err(|e| DslError::core(&e))
}

pub(super) fn deployment_animation_step(
    p: &mut Parser,
    view: ViewRef,
    tokens: &Tokens,
    start: usize,
) -> Result<(), DslError> {
    let mut instances: Vec<Id> = Vec::new();
    let mut infra: Vec<Id> = Vec::new();
    for i in start..tokens.size() {
        let token = tokens.get(i);
        let items = if is_expression(&token) {
            p.parse_expression(&token, ExprScope::Deployment)?
        } else {
            p.parse_identifier(&token, ExprScope::Deployment)?
        };
        for item in items {
            if let Mi::Element(id) = item {
                match p.builder.element_kind(&id) {
                    Some(ElementKind::SoftwareSystemInstance | ElementKind::ContainerInstance) => {
                        instances.push(id);
                    }
                    Some(ElementKind::InfrastructureNode) => infra.push(id),
                    _ => {}
                }
            }
        }
    }
    if instances.is_empty() && infra.is_empty() {
        return Err(DslError::stmt(
            "No software system instances, container instances, or infrastructure nodes were found",
        ));
    }
    // `DeploymentView.addAnimation(instances, infrastructureNodes)` —
    // The animation add takes
    instances.extend(infra);
    p.builder
        .view_add_animation_step(view, &instances)
        .map_err(|e| DslError::core(&e))
}

pub(super) fn custom_animation_step(
    p: &mut Parser,
    view: ViewRef,
    tokens: &Tokens,
    start: usize,
) -> Result<(), DslError> {
    let mut ids: Vec<Id> = Vec::new();
    for i in start..tokens.size() {
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
                ids.push(id);
            }
        }
    }
    if ids.is_empty() {
        return Err(DslError::stmt("No custom elements were found"));
    }
    p.builder
        .view_add_animation_step(view, &ids)
        .map_err(|e| DslError::core(&e))
}
