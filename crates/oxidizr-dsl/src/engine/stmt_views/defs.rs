//! View-definition arms of the view sections: the per-kind view parsers
//! (custom, landscape, context, container, component, dynamic, deployment,
//! filtered, image) with their grammar diagnostics. The content, animation,
//! step and configuration arms live in the sibling modules; the shared free
//! helpers stay in the parent module.

use oxidizr_core::validate::ElementKind;
use oxidizr_core::views::FilterMode;
use oxidizr_core::{Id, ViewRef};

use crate::compat::java_trim;

use super::super::idents::{PseudoKind, RegItem};
use super::{
    DslError, Parser, Tokens, apply_text, expected, is_trim_empty, too_many, validate_view_key,
};

// ---------------------------------------------------------------------------
// view definitions
// ---------------------------------------------------------------------------

/// the custom-view parser.
pub(super) fn custom_view(p: &mut Parser, t: &Tokens) -> Result<ViewRef, DslError> {
    const G: &str = "custom [key] [title] [description] {";
    if t.has_more_than(3) {
        return Err(too_many(G));
    }
    let mut key = String::new();
    let mut title = String::new();
    let mut description = String::new();
    if t.includes(1) {
        key = validate_view_key(t.get(1))?;
    }
    if t.includes(2) {
        title = t.get(2).into_owned();
    }
    if t.includes(3) {
        description = t.get(3).into_owned();
    }
    let view = p
        .builder
        .create_custom_view(&key, None)
        .map_err(|e| DslError::core(&e))?;
    // the custom-view parser defaults title/description to "" and the `CustomView`
    // constructor stores them VERBATIM — unlike the other view kinds, an empty
    // value IS stored. JSON still omits it (`NON_EMPTY`), but
    // `CustomView.getName` = `"Custom View: " + getTitle` makes the empty
    // title observable in the plantuml header (cross-checked: custom-view-animation).
    if let Some(base) = p.builder.view_base_mut(view) {
        base.title = Some(title);
        base.description = Some(description);
    }
    Ok(view)
}

/// the system-landscape-view parser.
pub(super) fn landscape_view(p: &mut Parser, t: &Tokens) -> Result<ViewRef, DslError> {
    const G: &str = "systemLandscape [key] [description] {";
    if t.has_more_than(2) {
        return Err(too_many(G));
    }
    let mut key = String::new();
    let mut description = String::new();
    if t.includes(1) {
        key = validate_view_key(t.get(1))?;
    }
    if t.includes(2) {
        description = t.get(2).into_owned();
    }
    let view = p
        .builder
        .create_landscape_view(&key)
        .map_err(|e| DslError::core(&e))?;
    apply_text(p, view, description, false);
    Ok(view)
}

/// the system-context-view parser — element resolution BEFORE key validation.
pub(super) fn context_view(p: &mut Parser, t: &Tokens) -> Result<ViewRef, DslError> {
    const G: &str = "systemContext <software system identifier> [key] [description] {";
    if t.has_more_than(3) {
        return Err(too_many(G));
    }
    if !t.includes(1) {
        return Err(expected(G));
    }
    let system = resolve_kind(
        p,
        &t.get(1),
        ElementKind::SoftwareSystem,
        &format!("The software system \"{}\" does not exist", t.get(1)),
        "The element \"%s\" is not a software system",
    )?;
    let mut key = String::new();
    let mut description = String::new();
    if t.includes(2) {
        key = validate_view_key(t.get(2))?;
    }
    if t.includes(3) {
        description = t.get(3).into_owned();
    }
    let view = p
        .builder
        .create_context_view(&system, &key)
        .map_err(|e| DslError::core(&e))?;
    apply_text(p, view, description, false);
    Ok(view)
}

/// the container-view parser.
pub(super) fn container_view(p: &mut Parser, t: &Tokens) -> Result<ViewRef, DslError> {
    const G: &str = "container <software system identifier> [key] [description] {";
    if t.has_more_than(3) {
        return Err(too_many(G));
    }
    if !t.includes(1) {
        return Err(expected(G));
    }
    let system = resolve_kind(
        p,
        &t.get(1),
        ElementKind::SoftwareSystem,
        &format!("The software system \"{}\" does not exist", t.get(1)),
        "The element \"%s\" is not a software system",
    )?;
    let mut key = String::new();
    let mut description = String::new();
    if t.includes(2) {
        key = validate_view_key(t.get(2))?;
    }
    if t.includes(3) {
        description = t.get(3).into_owned();
    }
    let view = p
        .builder
        .create_container_view(&system, &key)
        .map_err(|e| DslError::core(&e))?;
    apply_text(p, view, description, false);
    Ok(view)
}

/// the component-view parser.
pub(super) fn component_view(p: &mut Parser, t: &Tokens) -> Result<ViewRef, DslError> {
    const G: &str = "component <container identifier> [key] [description] {";
    if t.has_more_than(3) {
        return Err(too_many(G));
    }
    if !t.includes(1) {
        return Err(expected(G));
    }
    let container = resolve_kind(
        p,
        &t.get(1),
        ElementKind::Container,
        &format!("The container \"{}\" does not exist", t.get(1)),
        "The element \"%s\" is not a container",
    )?;
    let mut key = String::new();
    let mut description = String::new();
    if t.includes(2) {
        key = validate_view_key(t.get(2))?;
    }
    if t.includes(3) {
        description = t.get(3).into_owned();
    }
    let view = p
        .builder
        .create_component_view(&container, &key)
        .map_err(|e| DslError::core(&e))?;
    apply_text(p, view, description, false);
    Ok(view)
}

/// the lookup + kind-check pair: the `missing` text when the lookup
/// fails, the `not_a` text (with `%s` for the identifier) for a kind
/// mismatch — pseudo-elements fail the kind gate, not the lookup.
fn resolve_kind(
    p: &Parser,
    identifier: &str,
    kind: ElementKind,
    missing: &str,
    not_a: &str,
) -> Result<Id, DslError> {
    match p.get_element(identifier) {
        None => Err(DslError::stmt(missing.to_string())),
        Some(RegItem::Element(id)) if p.builder.element_kind(&id) == Some(kind) => Ok(id),
        Some(_) => Err(DslError::stmt(not_a.replace("%s", identifier))),
    }
}

/// the dynamic-view parser.
pub(super) fn dynamic_view(p: &mut Parser, t: &Tokens) -> Result<ViewRef, DslError> {
    const G: &str =
        "dynamic <*|software system identifier|container identifier> [key] [description] {";
    if t.has_more_than(3) {
        return Err(too_many(G));
    }
    if !t.includes(1) {
        return Err(expected(G));
    }
    let description = if t.includes(3) {
        t.get(3).into_owned()
    } else {
        String::new()
    };
    let scope = t.get(1);
    let scoped = if scope == "*" {
        None
    } else {
        match p.get_element(&scope) {
            None => {
                return Err(DslError::stmt(format!(
                    "The software system or container \"{scope}\" does not exist"
                )));
            }
            Some(RegItem::Element(id))
                if p.builder.is_system(&id) || p.builder.is_container(&id) =>
            {
                Some(id)
            }
            Some(_) => {
                return Err(DslError::stmt(format!(
                    "The element \"{scope}\" is not a software system or container"
                )));
            }
        }
    };
    let mut key = String::new();
    if t.includes(2) {
        key = validate_view_key(t.get(2))?;
    }
    let view = p
        .builder
        .create_dynamic_view(scoped, &key)
        .map_err(|e| DslError::core(&e))?;
    apply_text(p, view, description, false);
    Ok(view)
}

/// the deployment-view parser — the environment token resolves through a
/// registered environment pseudo (the official lookup by name),
/// else it is the environment name itself.
pub(super) fn deployment_view(p: &mut Parser, t: &Tokens) -> Result<ViewRef, DslError> {
    const G: &str = "deployment <*|software system identifier> <environment> [key] [description] {";
    if t.has_more_than(4) {
        return Err(too_many(G));
    }
    if !t.includes(2) {
        return Err(expected(G));
    }
    let scope = t.get(1);
    let env_token = t.get(2);
    let mut environment = env_token.clone().into_owned();
    if let Some(RegItem::Pseudo {
        kind: PseudoKind::DeploymentEnvironment(name),
        ..
    }) = p.get_element(&env_token)
    {
        environment = name;
    }
    let known = p
        .builder
        .workspace()
        .model
        .deployment_nodes
        .iter()
        .any(|dn| dn.environment == environment);
    if !known {
        return Err(DslError::stmt(format!(
            "The environment \"{environment}\" does not exist"
        )));
    }
    let description = if t.includes(4) {
        t.get(4).into_owned()
    } else {
        String::new()
    };
    let mut key = String::new();
    let view = if scope == "*" {
        if t.includes(3) {
            key = validate_view_key(t.get(3))?;
        }
        p.builder
            .create_deployment_view(&environment, &key)
            .map_err(|e| DslError::core(&e))?
    } else {
        let system = resolve_kind(
            p,
            &scope,
            ElementKind::SoftwareSystem,
            &format!("The software system \"{scope}\" does not exist"),
            "The element \"%s\" is not a software system",
        )?;
        if t.includes(3) {
            key = validate_view_key(t.get(3))?;
        }
        let view = p
            .builder
            .create_deployment_view(&environment, &key)
            .map_err(|e| DslError::core(&e))?;
        // The parser sets the deployment view scope.
        if let ViewRef::Deployment(i) = view
            && let Some(dv) = p.builder.views_mut().deployment_views.get_mut(i)
        {
            dv.software_system_id = Some(system);
        }
        view
    };
    apply_text(p, view, description, false);
    Ok(view)
}

/// the filtered-view parser. Tags split on `,` keeping LEADING empty parts
/// (the blank check TRIMS, so a `" "` tag is skipped too); the
/// the official hash-set order is not portable — insertion order, deduped.
pub(super) fn filtered_view(p: &mut Parser, t: &Tokens) -> Result<ViewRef, DslError> {
    const G: &str = "filtered <baseKey> <include|exclude> <tags> [key] [description]";
    if t.has_more_than(5) {
        return Err(too_many(G));
    }
    if !t.includes(3) {
        return Err(expected(G));
    }
    let base_key = t.get(1);
    let mode_token = t.get(2);
    let tags_as = t.get(3);
    let mut tags: Vec<String> = Vec::new();
    for tag in super::super::java_split_str(&tags_as, ",") {
        if !is_trim_empty(tag) {
            let tag = java_trim(tag).to_string();
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }
    }
    let description = if t.includes(5) {
        t.get(5).into_owned()
    } else {
        String::new()
    };
    let mode = if eqi_include(&mode_token) {
        FilterMode::Include
    } else if eqi_exclude(&mode_token) {
        FilterMode::Exclude
    } else {
        return Err(DslError::stmt("Filter mode should be include or exclude"));
    };
    let Some(base) = p.builder.view_ref_of_key(&base_key) else {
        return Err(DslError::stmt(format!(
            "The view \"{base_key}\" does not exist"
        )));
    };
    if !matches!(
        base,
        ViewRef::Landscape(_)
            | ViewRef::Context(_)
            | ViewRef::Container(_)
            | ViewRef::Component(_)
            | ViewRef::Deployment(_)
    ) {
        return Err(DslError::stmt(format!(
            "The view \"{base_key}\" must be a System Landscape, System Context, Container, Component, or Deployment view"
        )));
    }
    if p.builder
        .content_ref(base)
        .is_some_and(|c| c.automatic_layout.is_some())
    {
        return Err(DslError::stmt(format!(
            "The view \"{base_key}\" has automatic layout enabled - this is not supported for filtered views"
        )));
    }
    let mut key = String::new();
    if t.includes(4) {
        key = validate_view_key(t.get(4))?;
    }
    let view = p
        .builder
        .create_filtered_view(&base_key, &key, mode, tags)
        .map_err(|e| DslError::core(&e))?;
    apply_text(p, view, description, false);
    Ok(view)
}

fn eqi_include(s: &str) -> bool {
    super::super::eqi(s, "include")
}

fn eqi_exclude(s: &str) -> bool {
    super::super::eqi(s, "exclude")
}

/// the image-view parser. A pseudo-element (e.g. a `deploymentEnvironment`
/// identifier) never resolves as an image-view scope in the official parser
/// (verified): the `does not exist` text (erratum, a documented deviation).
pub(super) fn image_view_def(p: &mut Parser, t: &Tokens) -> Result<ViewRef, DslError> {
    const G: &str = "image <*|element identifier> [key] {";
    if t.has_more_than(2) {
        return Err(too_many(G));
    }
    if !t.includes(1) {
        return Err(expected(G));
    }
    let mut key = String::new();
    if t.includes(2) {
        key = validate_view_key(t.get(2))?;
    }
    let scope = t.get(1);
    let element = if scope == "*" {
        None
    } else {
        match p.get_element(&scope) {
            Some(RegItem::Element(id)) => Some(id),
            _ => {
                return Err(DslError::stmt(format!(
                    "The element \"{scope}\" does not exist"
                )));
            }
        }
    };
    p.builder
        .create_image_view(element, &key, None, None)
        .map_err(|e| DslError::core(&e))
}
