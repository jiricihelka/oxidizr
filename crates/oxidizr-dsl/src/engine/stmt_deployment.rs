//! Chain section: reference `StructurizrDslParser.java` arms at Java lines
//! 949–1027 — `deploymentEnvironment` (the name is a DSL-only
//! [`DeploymentEnvironment`] pseudo, only the nodes put the environment into
//! the model), the two flat `deploymentGroup` arms (NEITHER pushes a
//! context), the environment- and node-scope `deploymentNode` arms
//! (`DeploymentNodeParser`), `infrastructureNode`
//! (`InfrastructureNodeParser`), `instanceOf` (`InstanceOfParser`),
//! `softwareSystemInstance` / `containerInstance`
//! (`SoftwareSystemInstanceParser` / `ContainerInstanceParser`) and the
//! `healthCheck` statement (`HealthCheckParser`) inside an instance.
//!
//! [`DeploymentEnvironment`]: super::ctx::DslEnvironment

use oxidizr_core::Id;
use oxidizr_core::validate::ElementKind;

use super::ctx::{Ctx, DslEnvironment};
use super::error::DslError;
use super::rel::RelArch;
use super::{
    KindFilter, Parser, RegItem, Sec, Stmt, Tokens, deployment_group_pseudo, env_pseudo, eqi,
    java_split_str, tok,
};

#[allow(clippy::too_many_lines)] // the chain arm order IS the contract
pub(crate) fn section(p: &mut Parser, stmt: &mut Stmt) -> Sec {
    let first = stmt.first();
    let idr = stmt.identifier.clone();
    let idr = idr.as_deref();
    let raw = stmt.tokens.clone();
    let t = raw.without_context_start_token();
    let open = Parser::should_start_context(&raw);

    let is_model = p.in_ctx(|c| matches!(c, Ctx::Model { .. }));
    let is_env = p.in_ctx(Ctx::is_environment);
    let is_node = p.in_ctx(|c| matches!(c, Ctx::DeploymentNode { .. }));
    let is_instance =
        p.in_ctx(|c| matches!(c, Ctx::SsInstance { .. } | Ctx::ContainerInstance { .. }));

    // 949: deploymentEnvironment <name> { — the name registers a pseudo;
    // no environment object ever enters the model until a node is added.
    if eqi(&first, tok::DEPLOYMENT_ENVIRONMENT) && is_model {
        return (|| {
            let environment = parse_environment(&t)?;
            if open {
                p.start_context(Ctx::Environment {
                    env: DslEnvironment {
                        name: environment.clone(),
                    },
                    group: None,
                });
            }
            let uid = p.next_uid();
            p.register_identifier_element(idr, env_pseudo(uid, &environment), &environment)
        })()
        .into();
    }

    // 957/961: deploymentGroup — flat statements, NO context is ever
    // started (a trailing `{` therefore closes the ENCLOSING context on the
    // matching `}`, exactly like the reference).
    if eqi(&first, tok::DEPLOYMENT_GROUP) && (is_env || is_node) {
        return (|| {
            let env_name = match p.top() {
                Some(Ctx::Environment { env, .. }) => Some(env.name.clone()),
                Some(Ctx::NoRelInEnv { environment, .. }) => Some(environment.clone()),
                _ => None,
            };
            if let Some(env) = env_name {
                // 957: environment scope — register a pseudo under the
                // statement's identifier; nodes reference it later.
                let name = parse_group_name(&t)?;
                let uid = p.next_uid();
                p.register_identifier_element(idr, deployment_group_pseudo(uid, &env, &name), &name)
            } else {
                // 961: node scope — resolve the references NOW and attach
                // them to the deployment node (`DeploymentNode
                // .addDeploymentGroup`).
                let Some(id) = node_id(p) else {
                    return Ok(());
                };
                parse_node_groups(&t)?;
                let groups = p.deployment_groups(&id, &t.get(1));
                for group in &groups {
                    p.builder
                        .add_deployment_group(&id, group)
                        .map_err(|e| DslError::core(&e))?;
                }
                Ok(())
            }
        })()
        .into();
    }

    // 965/974: deploymentNode — environment scope creates a ROOT node, node
    // scope a child. Archetype defaults fill description/technology/tags.
    if p.is_element_keyword_or_archetype(&first, tok::DEPLOYMENT_NODE) && (is_env || is_node) {
        return (|| {
            let arch = p.get_archetype(tok::DEPLOYMENT_NODE, &first);
            let id = match p.top().cloned() {
                Some(Ctx::Environment { env, .. }) => {
                    def_deployment_node(p, &t, Some(&env.name), None, arch)?
                }
                Some(Ctx::NoRelInEnv { environment, .. }) => {
                    def_deployment_node(p, &t, Some(&environment), None, arch)?
                }
                Some(Ctx::DeploymentNode { id, .. }) => {
                    def_deployment_node(p, &t, None, Some(&id), arch)?
                }
                _ => return Ok(()),
            };
            if open {
                // Java pushes a PLAIN context — the group payload belongs to
                // the NEW node's own children: none.
                p.start_context(Ctx::DeploymentNode {
                    id: id.clone(),
                    group: None,
                });
            }
            let canonical = p.builder.canonical_name_full(&id).unwrap_or_default();
            p.register_identifier_element(idr, RegItem::Element(id), &canonical)
        })()
        .into();
    }

    // 983: infrastructureNode <name> [description] [technology] [tags].
    if p.is_element_keyword_or_archetype(&first, tok::INFRASTRUCTURE_NODE) && is_node {
        return (|| {
            let Some(node) = node_id(p) else {
                return Ok(());
            };
            let arch = p.get_archetype(tok::INFRASTRUCTURE_NODE, &first);
            let id = def_infrastructure_node(p, &node, &t, arch)?;
            if open {
                p.start_context(Ctx::InfrastructureNode { id: id.clone() });
            }
            let canonical = p.builder.canonical_name_full(&id).unwrap_or_default();
            p.register_identifier_element(idr, RegItem::Element(id), &canonical)
        })()
        .into();
    }

    // 1002: instanceOf <identifier> — dispatches on the resolved element's
    // kind; the created instance type picks the context.
    if eqi(&first, tok::INSTANCE_OF) && is_node {
        return (|| {
            let Some(node) = node_id(p) else {
                return Ok(());
            };
            if t.has_more_than(3) {
                return Err(DslError::stmt(
                    "Too many tokens, expected: instanceOf <identifier> [deploymentGroups] [tags]",
                ));
            }
            if !t.includes(1) {
                return Err(DslError::stmt(
                    "Expected: instanceOf <identifier> [deploymentGroups] [tags]",
                ));
            }
            let element_identifier = t.get(1);
            let kind = match p.get_element(&element_identifier) {
                Some(RegItem::Element(id)) => p.builder.element_kind(&id),
                // DSL pseudo-elements are `Element`s too — they fall into
                // the same "not a software system or a container" error.
                Some(_) => None,
                None => {
                    return Err(DslError::stmt(format!(
                        "The element \"{element_identifier}\" does not exist"
                    )));
                }
            };
            let (id, system) = match kind {
                Some(ElementKind::SoftwareSystem) => (def_ss_instance(p, &node, &t)?, true),
                Some(ElementKind::Container) => (def_container_instance(p, &node, &t)?, false),
                _ => {
                    return Err(DslError::stmt(format!(
                        "The element \"{element_identifier}\" must be a software system or a container"
                    )));
                }
            };
            if open {
                if system {
                    p.start_context(Ctx::SsInstance { id: id.clone() });
                } else {
                    p.start_context(Ctx::ContainerInstance { id: id.clone() });
                }
            }
            let canonical = p.builder.canonical_name_full(&id).unwrap_or_default();
            p.register_identifier_element(idr, RegItem::Element(id), &canonical)
        })()
        .into();
    }

    // 1018: softwareSystemInstance <identifier> [deploymentGroups] [tags].
    if eqi(&first, tok::SOFTWARE_SYSTEM_INSTANCE) && is_node {
        return (|| {
            let Some(node) = node_id(p) else {
                return Ok(());
            };
            let id = def_ss_instance(p, &node, &t)?;
            if open {
                p.start_context(Ctx::SsInstance { id: id.clone() });
            }
            let canonical = p.builder.canonical_name_full(&id).unwrap_or_default();
            p.register_identifier_element(idr, RegItem::Element(id), &canonical)
        })()
        .into();
    }

    // 1025: containerInstance <identifier> [deploymentGroups] [tags].
    if eqi(&first, tok::CONTAINER_INSTANCE) && is_node {
        return (|| {
            let Some(node) = node_id(p) else {
                return Ok(());
            };
            let id = def_container_instance(p, &node, &t)?;
            if open {
                p.start_context(Ctx::ContainerInstance { id: id.clone() });
            }
            let canonical = p.builder.canonical_name_full(&id).unwrap_or_default();
            p.register_identifier_element(idr, RegItem::Element(id), &canonical)
        })()
        .into();
    }

    // 1027: healthCheck <name> <url> [interval] [timeout] — inside a
    // software-system or container instance.
    if eqi(&first, tok::HEALTH_CHECK) && is_instance {
        return (|| {
            let id = match p.top() {
                Some(Ctx::SsInstance { id } | Ctx::ContainerInstance { id }) => id.clone(),
                _ => return Ok(()),
            };
            if t.has_more_than(4) {
                return Err(DslError::stmt(
                    "Too many tokens, expected: healthCheck <name> <url> [interval] [timeout]",
                ));
            }
            if !t.includes(2) {
                return Err(DslError::stmt(
                    "Expected: healthCheck <name> <url> [interval] [timeout]",
                ));
            }
            let name = t.get(1);
            let url = t.get(2);
            // `HealthCheckParser`: defaults interval 60 s / timeout 0 ms;
            // interval ≥ 1, timeout ≥ 0, both via `Integer.parseInt` (the
            // `RuntimeException` range errors pass through the NFE catch).
            let mut interval = 60;
            let mut timeout = 0;
            if t.includes(3) {
                let raw_interval = t.get(3);
                interval = crate::compat::java_parse_int(&raw_interval).ok_or_else(|| {
                    DslError::stmt(format!(
                        "The interval of \"{raw_interval}\" is not valid - it must be a positive integer (number of seconds)"
                    ))
                })?;
                if interval < 1 {
                    return Err(DslError::stmt(
                        "The interval must be a positive integer (number of seconds)",
                    ));
                }
            }
            if t.includes(4) {
                let raw_timeout = t.get(4);
                let parsed = crate::compat::java_parse_int(&raw_timeout).ok_or_else(|| {
                    DslError::stmt(format!(
                        "The timeout of \"{raw_timeout}\" is not valid - it must be zero or a positive integer (number of milliseconds)"
                    ))
                })?;
                if parsed < 0 {
                    return Err(DslError::stmt(
                        "The timeout must be zero or a positive integer (number of milliseconds)",
                    ));
                }
                timeout = i64::from(parsed);
            }
            p.builder
                .add_health_check(&id, &name, &url, interval, timeout)
                .map_err(|e| DslError::core(&e))
        })()
        .into();
    }

    Sec::No
}

// ----------------------------------------------------------------------
// helpers
// ----------------------------------------------------------------------

/// The node id of a `DeploymentNode` context (`None` only when the chain
/// guard cannot fire — the arms then become no-ops, like the reference's
/// `getContext()` never failing).
fn node_id(p: &Parser) -> Option<Id> {
    match p.top() {
        Some(Ctx::DeploymentNode { id, .. }) => Some(id.clone()),
        _ => None,
    }
}

/// `DeploymentEnvironmentParser.parse`.
fn parse_environment(tokens: &Tokens) -> Result<String, DslError> {
    if tokens.has_more_than(1) {
        return Err(DslError::stmt(
            "Too many tokens, expected: deploymentEnvironment <name> {",
        ));
    }
    // `tokens.size() != 2` — not merely `!includes(1)`.
    if tokens.size() != 2 {
        return Err(DslError::stmt("Expected: deploymentEnvironment <name> {"));
    }
    Ok(tokens.get(1))
}

/// `DeploymentGroupParser.parse` (environment scope).
fn parse_group_name(tokens: &Tokens) -> Result<String, DslError> {
    if tokens.has_more_than(1) {
        return Err(DslError::stmt(
            "Too many tokens, expected: deploymentGroup <name>",
        ));
    }
    if tokens.size() != 2 {
        return Err(DslError::stmt("Expected: deploymentGroup <name>"));
    }
    Ok(tokens.get(1))
}

/// `DeploymentGroupParser.parse` (node scope) — same guards, different
/// grammar text.
fn parse_node_groups(tokens: &Tokens) -> Result<(), DslError> {
    if tokens.has_more_than(1) {
        return Err(DslError::stmt(
            "Too many tokens, expected: deploymentGroup <deploymentGroups>",
        ));
    }
    if tokens.size() != 2 {
        return Err(DslError::stmt(
            "Expected: deploymentGroup <deploymentGroups>",
        ));
    }
    Ok(())
}

/// `DeploymentNodeParser.parse` — root (`environment`) or child (`parent`).
fn def_deployment_node(
    p: &mut Parser<'_>,
    tokens: &Tokens,
    environment: Option<&str>,
    parent: Option<&Id>,
    arch: usize,
) -> Result<Id, DslError> {
    const GRAMMAR: &str = "deploymentNode <name> [description] [technology] [tags] [instances] {";
    if tokens.has_more_than(5) {
        return Err(DslError::stmt(format!(
            "Too many tokens, expected: {GRAMMAR}"
        )));
    }
    if !tokens.includes(1) {
        return Err(DslError::stmt(format!("Expected: {GRAMMAR}")));
    }
    let a = p.arch_snapshot(arch);
    let name = tokens.get(1);
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
    let id = match parent {
        None => p
            .builder
            .add_deployment_node(
                environment.unwrap_or_default(),
                &name,
                Some(description.clone()),
                Some(technology.clone()),
                None,
            )
            .map_err(|e| DslError::core(&e))?,
        Some(parent) => p
            .builder
            .add_child_deployment_node(
                parent,
                &name,
                Some(description.clone()),
                Some(technology.clone()),
                None,
            )
            .map_err(|e| DslError::core(&e))?,
    };
    let tags = arch_tags(&a, tokens.includes(4).then(|| tokens.get(4)));
    // `addProperties` / `addPerspectives` / tags / group — the shared
    // post-creation sequence (`instances` is applied by the caller BEFORE
    // registration, matching the reference order inside the parser).
    p.define_element(&id, &description, &tags, &a)?;
    if tokens.includes(5) {
        // `setInstances` stores the raw value (validated); the node ctor
        // already carries the default "1".
        let instances = tokens.get(5);
        p.builder
            .set_instances(&id, &instances)
            .map_err(|e| DslError::core(&e))?;
    }
    Ok(id)
}

/// `InfrastructureNodeParser.parse`.
fn def_infrastructure_node(
    p: &mut Parser<'_>,
    node: &Id,
    tokens: &Tokens,
    arch: usize,
) -> Result<Id, DslError> {
    const GRAMMAR: &str = "infrastructureNode <name> [description] [technology] [tags]";
    if tokens.has_more_than(4) {
        return Err(DslError::stmt(format!(
            "Too many tokens, expected: {GRAMMAR}"
        )));
    }
    if !tokens.includes(1) {
        return Err(DslError::stmt(format!("Expected: {GRAMMAR}")));
    }
    let a = p.arch_snapshot(arch);
    let name = tokens.get(1);
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
    let id = p
        .builder
        .add_infrastructure_node(
            node,
            &name,
            Some(description.clone()),
            Some(technology.clone()),
            None,
        )
        .map_err(|e| DslError::core(&e))?;
    let tags = arch_tags(&a, tokens.includes(4).then(|| tokens.get(4)));
    p.define_element(&id, &description, &tags, &a)?;
    Ok(id)
}

/// `SoftwareSystemInstanceParser.parse` (also the `instanceOf` dispatch
/// target — the reference re-parses the SAME tokens there).
fn def_ss_instance(p: &mut Parser<'_>, node: &Id, tokens: &Tokens) -> Result<Id, DslError> {
    const GRAMMAR: &str = "softwareSystemInstance <identifier> [deploymentGroups] [tags]";
    if tokens.has_more_than(3) {
        return Err(DslError::stmt(format!(
            "Too many tokens, expected: {GRAMMAR}"
        )));
    }
    if !tokens.includes(1) {
        return Err(DslError::stmt(format!("Expected: {GRAMMAR}")));
    }
    let software_system_identifier = tokens.get(1);
    let Some(RegItem::Element(system)) = p.get_element_typed(
        &software_system_identifier,
        Some(KindFilter::SoftwareSystem),
    ) else {
        return Err(DslError::stmt(format!(
            "The software system \"{software_system_identifier}\" does not exist"
        )));
    };
    let groups = if tokens.includes(2) {
        p.deployment_groups(node, &tokens.get(2))
    } else {
        Vec::new()
    };
    let id = p
        .builder
        .add_software_system_instance(node, &system, None, groups)
        .map_err(|e| DslError::core(&e))?;
    if tokens.includes(3) {
        let tags = tokens.get(3);
        let parts: Vec<String> = java_split_str(&tags, ",")
            .into_iter()
            .map(str::to_string)
            .collect();
        p.builder.add_tag_parts(&id, &parts);
    }
    apply_group(p, &id);
    Ok(id)
}

/// `ContainerInstanceParser.parse`.
fn def_container_instance(p: &mut Parser<'_>, node: &Id, tokens: &Tokens) -> Result<Id, DslError> {
    const GRAMMAR: &str = "containerInstance <identifier> [deploymentGroups] [tags]";
    if tokens.has_more_than(3) {
        return Err(DslError::stmt(format!(
            "Too many tokens, expected: {GRAMMAR}"
        )));
    }
    if !tokens.includes(1) {
        return Err(DslError::stmt(format!("Expected: {GRAMMAR}")));
    }
    let container_identifier = tokens.get(1);
    let Some(RegItem::Element(container)) =
        p.get_element_typed(&container_identifier, Some(KindFilter::Container))
    else {
        return Err(DslError::stmt(format!(
            "The container \"{container_identifier}\" does not exist"
        )));
    };
    let groups = if tokens.includes(2) {
        p.deployment_groups(node, &tokens.get(2))
    } else {
        Vec::new()
    };
    let id = p
        .builder
        .add_container_instance(node, &container, None, groups)
        .map_err(|e| DslError::core(&e))?;
    if tokens.includes(3) {
        let tags = tokens.get(3);
        let parts: Vec<String> = java_split_str(&tags, ",")
            .into_iter()
            .map(str::to_string)
            .collect();
        p.builder.add_tag_parts(&id, &parts);
    }
    apply_group(p, &id);
    Ok(id)
}

/// The `tags` seed: archetype tags plus the comma-split token
/// (Java `split` drops trailing empties).
fn arch_tags(a: &RelArch, tags_token: Option<String>) -> Vec<String> {
    let mut tags = a.tags.clone();
    if let Some(csv) = tags_token {
        tags.extend(java_split_str(&csv, ",").into_iter().map(str::to_string));
    }
    tags
}

/// The `context.hasGroup()` tail shared by the instance parsers:
/// `setGroup` + `group.addElement`. (Nodes and infrastructure nodes get it
/// via [`Parser::define_element`].)
fn apply_group(p: &mut Parser<'_>, id: &Id) {
    let group = p.top().and_then(Ctx::group_payload).cloned();
    if let Some(group) = group {
        let _ = p.builder.set_group(id, &group.name);
        p.group_add_member(group.uid, id);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::super::{ParseOptions, ParseResult, parse_dsl};

    fn full(dsl: &str) -> ParseResult {
        parse_dsl(dsl, ParseOptions::fragment()).expect("parse")
    }

    fn errors(r: &ParseResult) -> Vec<String> {
        r.errors.iter().map(|e| e.message.clone()).collect()
    }

    fn assert_clean(r: &ParseResult) {
        assert!(r.errors.is_empty(), "unexpected errors: {:?}", errors(r));
    }

    const MODEL_HEAD: &str = r#"
workspace {
  model {
    system = softwareSystem "System" {
      c = container "C"
    }
"#;

    #[test]
    fn environment_node_instance_health_check() {
        let r = full(&format!(
            r#"{MODEL_HEAD}
    deploymentEnvironment "Live" {{
      n1 = deploymentNode "Node 1" "desc" "AWS" "T1, T2" "2" {{
        inst1 = softwareSystemInstance system {{
          healthCheck "hc" "http://example.com/hc" 30 500
        }}
        ci = containerInstance c "dg1" "Extra"
      }}
    }}
  }}
}}
"#
        ));
        assert_clean(&r);
        let w = &r.workspace;
        let node = &w.model.deployment_nodes[0];
        assert_eq!(node.environment, "Live");
        assert_eq!(node.item.name, "Node 1");
        assert_eq!(node.item.description.as_deref(), Some("desc"));
        assert_eq!(node.technology.as_deref(), Some("AWS"));
        assert_eq!(node.instances, "2");
        assert_eq!(node.item.tags.stored(), ["T1", "T2"]);
        let inst = &node.software_system_instances[0];
        assert_eq!(inst.instance_id, 1);
        assert_eq!(inst.item.tags.stored(), ["Software System Instance"]);
        let hc = &inst.health_checks[0];
        assert_eq!(hc.name, "hc");
        assert_eq!(hc.url, "http://example.com/hc");
        assert_eq!(hc.interval, 30);
        assert_eq!(hc.timeout, 500);
        let ci = &node.container_instances[0];
        assert_eq!(ci.item.name, "C");
        assert_eq!(ci.item.tags.stored(), ["Container Instance", "Extra"]);
    }

    #[test]
    fn default_instances_and_health_check_defaults() {
        let r = full(&format!(
            r#"{MODEL_HEAD}
    deploymentEnvironment "Live" {{
      deploymentNode "N" {{
        softwareSystemInstance system {{
          healthCheck "hc" "https://example.com"
        }}
      }}
    }}
  }}
}}
"#
        ));
        assert_clean(&r);
        let node = &r.workspace.model.deployment_nodes[0];
        assert_eq!(node.instances, "1", "ctor default");
        let hc = &node.software_system_instances[0].health_checks[0];
        assert_eq!(hc.interval, 60);
        assert_eq!(hc.timeout, 0);
    }

    #[test]
    fn hierarchical_identifiers_use_the_environment_prefix() {
        // Oracle-verified (`deploy-id.dsl` probe): a named environment
        // prefixes node/instance identifiers (`live.n1`), an anonymous one
        // prefixes with its registration UUID.
        let r = full(
            r#"workspace {
  !identifiers hierarchical
  model {
    system = softwareSystem "System"
    live = deploymentEnvironment "Live" {
      n1 = deploymentNode "Node 1" {
        inst1 = softwareSystemInstance system
      }
    }
  }
}
"#,
        );
        assert_clean(&r);
        let w = &r.workspace;
        let node = &w.model.deployment_nodes[0];
        assert_eq!(
            node.item.properties.get("structurizr.dsl.identifier"),
            Some("live.n1")
        );
        let inst = &node.software_system_instances[0];
        assert_eq!(
            inst.item.properties.get("structurizr.dsl.identifier"),
            Some("live.n1.inst1")
        );
    }

    #[test]
    fn deployment_groups_flat_and_inherited() {
        // Mirrors `deployment-groups-flat.dsl`: named pseudo registration in
        // the environment, instance-level references, and node-level
        // inheritance via `deploymentGroup <ref>`.
        let r = full(&format!(
            r#"{MODEL_HEAD}
    deploymentEnvironment "E" {{
      serviceInstance1 = deploymentGroup "Service Instance 1"
      deploymentNode "Server 1" {{
        deploymentGroup serviceInstance1
        containerInstance c
        containerInstance c serviceInstance1
      }}
    }}
  }}
}}
"#
        ));
        assert_clean(&r);
        let node = &r.workspace.model.deployment_nodes[0];
        assert_eq!(node.deployment_groups, ["Service Instance 1"]);
        let inherited = &node.container_instances[0];
        let referenced = &node.container_instances[1];
        assert_eq!(inherited.deployment_groups, [] as [String; 0]);
        assert_eq!(referenced.deployment_groups, ["Service Instance 1"]);
    }

    #[test]
    fn deployment_group_reference_resolves_via_environment_identifier() {
        // The hierarchical fixture shape: inside a node the bare ref misses
        // the `<nodeId>.<ref>` walk, so the `<envIdentifier>.<ref>` fallback
        // resolves it (the environment pseudo equals by name).
        let r = full(
            r#"workspace {
  !identifiers hierarchical
  model {
    softwareSystem = softwareSystem "System" {
      api = container "API"
    }
    e = deploymentEnvironment "Env" {
      serviceInstance1 = deploymentGroup "Service Instance 1"
      deploymentNode "Server 1" {
        containerInstance softwareSystem.api serviceInstance1
      }
    }
  }
}
"#,
        );
        assert_clean(&r);
        let node = &r.workspace.model.deployment_nodes[0];
        assert_eq!(
            node.container_instances[0].deployment_groups,
            ["Service Instance 1"]
        );
    }

    #[test]
    fn instance_of_dispatch_and_errors() {
        let r = full(&format!(
            r#"{MODEL_HEAD}
    deploymentEnvironment "E" {{
      deploymentNode "N" {{
        s = instanceOf system
        dc = instanceOf c
        bad = instanceOf c d e f
      }}
    }}
  }}
}}
"#
        ));
        assert_eq!(
            errors(&r),
            vec!["Too many tokens, expected: instanceOf <identifier> [deploymentGroups] [tags]"]
        );
        let node = &r.workspace.model.deployment_nodes[0];
        assert_eq!(node.software_system_instances.len(), 1);
        assert_eq!(node.container_instances.len(), 1);

        // Missing element / wrong kind — single-pass order: `s2` must be
        // declared before the `instanceOf` statement.
        let r = full(&format!(
            r#"{MODEL_HEAD}
    s2 = softwareSystem "S2" {{
      cont = container "K"
    }}
    deploymentEnvironment "E" {{
      deploymentNode "N" {{
        instanceOf nope
        instanceOf s2
      }}
    }}
  }}
}}
"#
        ));
        assert_eq!(
            errors(&r),
            vec!["The element \"nope\" does not exist"],
            "a plain softwareSystem IS valid for instanceOf"
        );

        let r = full(
            r#"workspace {
  model {
    s = softwareSystem "S" {
      c = container "C" {
        k = component "K"
      }
    }
    deploymentEnvironment "E" {
      deploymentNode "N" {
        instanceOf k
      }
    }
  }
}
"#,
        );
        assert_eq!(
            errors(&r),
            vec!["The element \"k\" must be a software system or a container"]
        );
    }

    #[test]
    fn instance_grammar_and_lookup_errors() {
        let r = full(&format!(
            r#"{MODEL_HEAD}
    deploymentEnvironment "E" {{
      deploymentNode "N" {{
        softwareSystemInstance
        softwareSystemInstance system x y z
        softwareSystemInstance nope
        containerInstance system
      }}
    }}
  }}
}}
"#
        ));
        assert_eq!(
            errors(&r),
            vec![
                "Expected: softwareSystemInstance <identifier> [deploymentGroups] [tags]",
                "Too many tokens, expected: softwareSystemInstance <identifier> [deploymentGroups] [tags]",
                "The software system \"nope\" does not exist",
                "The container \"system\" does not exist",
            ]
        );
    }

    #[test]
    fn deployment_environment_and_node_guards() {
        let r = full(
            r"workspace {
  model {
    deploymentEnvironment
    deploymentEnvironment A B {
    }
  }
}
",
        );
        assert_eq!(
            errors(&r),
            vec![
                "Expected: deploymentEnvironment <name> {",
                "Too many tokens, expected: deploymentEnvironment <name> {",
                // the failed `deploymentEnvironment A B {` never pushed a context,
                // so its `}` closes `model`, and the next two `}`s over-close.
                "Unexpected end of context",
            ]
        );

        let r = full(
            r#"workspace {
  model {
    deploymentEnvironment "E" {
      deploymentNode
      deploymentNode "A" "d" "t" "g" "1" "2"
    }
  }
}
"#,
        );
        assert_eq!(
            errors(&r),
            vec![
                "Expected: deploymentNode <name> [description] [technology] [tags] [instances] {",
                "Too many tokens, expected: deploymentNode <name> [description] [technology] [tags] [instances] {",
            ]
        );
    }

    #[test]
    fn node_group_statement_errors_and_dedup() {
        let r = full(&format!(
            r#"{MODEL_HEAD}
    deploymentEnvironment "E" {{
      g1 = deploymentGroup "G One"
      g2 = deploymentGroup "G Two"
      deploymentNode "N" {{
        deploymentGroup
        deploymentGroup g1 a b
      }}
      g3 = deploymentGroup
    }}
  }}
}}
"#
        ));
        assert_eq!(
            errors(&r),
            vec![
                "Expected: deploymentGroup <deploymentGroups>",
                "Too many tokens, expected: deploymentGroup <deploymentGroups>",
                "Expected: deploymentGroup <name>",
            ]
        );
    }

    #[test]
    fn health_check_validation_texts() {
        let r = full(&format!(
            r#"{MODEL_HEAD}
    deploymentEnvironment "E" {{
      deploymentNode "N" {{
        i = softwareSystemInstance system {{
          healthCheck
          healthCheck "a" "b" "c" "d" "e"
          healthCheck "a" "http://x" zero
          healthCheck "a" "http://x" 0
          healthCheck "a" "http://x" 1 -2
          healthCheck "" "http://x"
          healthCheck "a" "" 
          healthCheck "a" "not a url"
        }}
      }}
    }}
  }}
}}
"#
        ));
        assert_eq!(
            errors(&r),
            vec![
                "Expected: healthCheck <name> <url> [interval] [timeout]",
                "Too many tokens, expected: healthCheck <name> <url> [interval] [timeout]",
                "The interval of \"zero\" is not valid - it must be a positive integer (number of seconds)",
                "The interval must be a positive integer (number of seconds)",
                "The timeout must be zero or a positive integer (number of milliseconds)",
                "The name must not be null or empty.",
                "The URL must not be null or empty.",
                "not a url is not a valid URL.",
            ]
        );
    }

    #[test]
    fn infrastructure_node_and_child_nodes() {
        let r = full(&format!(
            r#"{MODEL_HEAD}
    deploymentEnvironment "E" {{
      parent = deploymentNode "P" {{
        child = deploymentNode "C" {{
          lb = infrastructureNode "LB" "load balancer" "nginx"
        }}
      }}
    }}
  }}
}}
"#
        ));
        assert_clean(&r);
        let parent = &r.workspace.model.deployment_nodes[0];
        assert_eq!(parent.item.name, "P");
        let child = &parent.children[0];
        assert_eq!(child.item.name, "C");
        let lb = &child.infrastructure_nodes[0];
        assert_eq!(lb.item.name, "LB");
        assert_eq!(lb.item.description.as_deref(), Some("load balancer"));
        assert_eq!(lb.technology.as_deref(), Some("nginx"));
    }

    #[test]
    fn duplicate_top_level_node_name_errors() {
        let r = full(
            r#"workspace {
  model {
    deploymentEnvironment "E" {
      deploymentNode "N"
      deploymentNode "N"
    }
  }
}
"#,
        );
        assert_eq!(
            errors(&r),
            vec![
                "A top-level deployment node named 'N' already exists for the environment named 'E'."
            ]
        );
    }

    #[test]
    fn instance_relationships_replicate() {
        // `Model.addSoftwareSystemInstance` replicates the element's
        // relationships onto the instance (core `replicate_element_relationships`).
        let r = full(
            r#"workspace {
  model {
    a = softwareSystem "A"
    b = softwareSystem "B"
    a -> b "Uses"
    deploymentEnvironment "E" {
      deploymentNode "N" {
        softwareSystemInstance a
        softwareSystemInstance b
      }
    }
  }
}
"#,
        );
        assert_clean(&r);
        let w = &r.workspace;
        // Java `replicateElementRelationships`: creating instA replicates
        // nothing (no other instance exists yet); creating instB iterates
        // {instA} and copies the single `a -> b` relationship as
        // instA -> instB — exactly ONE replicated relationship, with a
        // `linkedRelationshipId` back to the original.
        assert_eq!(w.model.relationships.len(), 2, "instance rels replicated");
        let base = &w.model.relationships[0];
        let replicated = &w.model.relationships[1];
        assert_eq!(replicated.linked_relationship_id.as_ref(), Some(&base.id));
    }
}
