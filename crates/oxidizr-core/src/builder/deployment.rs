//! Deployment/instance creation legs of [`WorkspaceBuilder`]: deployment
//! nodes, infrastructure nodes, software-system/container instances,
//! deployment groups and health checks. The shared node-walk helpers stay in
//! the parent module.

#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use crate::error::CoreError;
use crate::ids::Id;
use crate::model::{ContainerInstance, InfrastructureNode, SoftwareSystemInstance};

use super::{WorkspaceBuilder, check_name, clean_groups, new_node};

impl WorkspaceBuilder {
    /// `deploymentNode "Name" …` as a top-level node in `environment`.
    pub fn add_deployment_node(
        &mut self,
        environment: &str,
        name: &str,
        description: Option<String>,
        technology: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        check_name(name)?;
        if self
            .workspace
            .model
            .deployment_nodes
            .iter()
            .any(|n| n.item.name == name && n.environment == environment)
        {
            return Err(CoreError::DuplicateDeploymentNodeName {
                name: name.to_string(),
                environment: environment.to_string(),
            });
        }
        let item = self.new_item(name, description, tags_csv);
        let id = item.id.clone();
        self.workspace
            .model
            .deployment_nodes
            .push(new_node(item, environment, technology));
        Ok(id)
    }

    /// A child `deploymentNode` under `parent`.
    pub fn add_child_deployment_node(
        &mut self,
        parent: &Id,
        name: &str,
        description: Option<String>,
        technology: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        check_name(name)?;
        let env = {
            let p = self
                .node_mut(parent)
                .ok_or_else(|| CoreError::DeploymentNodeNotFound { id: parent.clone() })?;
            // The official add rule: the child name must be
            // unique among the parent's child nodes AND infrastructure nodes.
            if p.children.iter().any(|c| c.item.name == name)
                || p.infrastructure_nodes.iter().any(|i| i.item.name == name)
            {
                return Err(CoreError::DuplicateChildDeploymentNodeName {
                    name: name.to_string(),
                    parent: p.item.name.clone(),
                });
            }
            p.environment.clone()
        };
        let item = self.new_item(name, description, tags_csv);
        let id = item.id.clone();
        // Re-lookup surfaces the same `DeploymentNodeNotFound` the guard
        // above raises — the old `expect("just found")` panic is gone
        // (nothing mutates the model between the lookups).
        self.node_mut(parent)
            .ok_or_else(|| CoreError::DeploymentNodeNotFound { id: parent.clone() })?
            .children
            .push(new_node(item, &env, technology));
        Ok(id)
    }

    /// `infrastructureNode "Name" ["Desc"] ["Tech"] ["Tags"]` under `node`.
    pub fn add_infrastructure_node(
        &mut self,
        node: &Id,
        name: &str,
        description: Option<String>,
        technology: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        check_name(name)?;
        let environment = {
            let parent = self
                .node_ref(node)
                .ok_or_else(|| CoreError::DeploymentNodeNotFound { id: node.clone() })?;
            // The official add rule: the name must be unique among the
            // parent's CHILD NODES and INFRASTRUCTURE NODES alike.
            if parent.children.iter().any(|c| c.item.name == name)
                || parent
                    .infrastructure_nodes
                    .iter()
                    .any(|i| i.item.name == name)
            {
                return Err(CoreError::DuplicateInfrastructureNodeName {
                    name: name.to_string(),
                    parent: parent.item.name.clone(),
                });
            }
            parent.environment.clone()
        };
        let item = self.new_item(name, description, tags_csv);
        let id = item.id.clone();
        // Error-valued re-lookup, as in `add_container`.
        self.node_mut(node)
            .ok_or_else(|| CoreError::DeploymentNodeNotFound { id: node.clone() })?
            .infrastructure_nodes
            .push(InfrastructureNode {
                item,
                group: None,
                environment,
                technology,
                relationships: Vec::new(),
            });
        Ok(id)
    }

    /// `softwareSystemInstance <systemId> …` under `node`.
    pub fn add_software_system_instance(
        &mut self,
        node: &Id,
        system: &Id,
        tags_csv: Option<&str>,
        deployment_groups: Vec<String>,
    ) -> Result<Id, CoreError> {
        if !self.is_system(system) {
            return Err(CoreError::WrongElementKind {
                id: system.clone(),
                expected: "software system",
            });
        }
        // The instance NAME is derived (never serialized in the format);
        // we mirror it for diagnostics.
        // `is_system` above guarantees the lookup; error-valued, not a panic
        // (same error the guard raises).
        let name = self.item(system).map(|i| i.name.clone()).ok_or_else(|| {
            CoreError::WrongElementKind {
                id: system.clone(),
                expected: "software system",
            }
        })?;
        let mut item = self.new_item(&name, None, tags_csv);
        item.tags.add(crate::tags::tag::SOFTWARE_SYSTEM_INSTANCE);
        let id = item.id.clone();
        let node = self
            .node_mut(node)
            .ok_or_else(|| CoreError::DeploymentNodeNotFound { id: node.clone() })?;
        // instanceId counts instances of the
        // SAME system within this node (1-based).
        let instance_id = node
            .software_system_instances
            .iter()
            .filter(|i| &i.software_system_id == system)
            .count()
            .saturating_add(1)
            .try_into()
            .unwrap_or(i32::MAX);
        let environment = node.environment.clone();
        node.software_system_instances.push(SoftwareSystemInstance {
            item,
            group: None,
            environment,
            software_system_id: system.clone(),
            instance_id,
            deployment_groups: clean_groups(deployment_groups),
            health_checks: Vec::new(),
            relationships: Vec::new(),
        });
        self.replicate_element_relationships(&id);
        Ok(id)
    }

    /// `containerInstance <containerId> …` under `node`.
    pub fn add_container_instance(
        &mut self,
        node: &Id,
        container: &Id,
        tags_csv: Option<&str>,
        deployment_groups: Vec<String>,
    ) -> Result<Id, CoreError> {
        if !self.is_container(container) {
            return Err(CoreError::WrongElementKind {
                id: container.clone(),
                expected: "container",
            });
        }
        // As in `add_software_system_instance`: guarded lookup, error-valued.
        let name = self
            .item(container)
            .map(|i| i.name.clone())
            .ok_or_else(|| CoreError::WrongElementKind {
                id: container.clone(),
                expected: "container",
            })?;
        let mut item = self.new_item(&name, None, tags_csv);
        item.tags.add(crate::tags::tag::CONTAINER_INSTANCE);
        let id = item.id.clone();
        let node = self
            .node_mut(node)
            .ok_or_else(|| CoreError::DeploymentNodeNotFound { id: node.clone() })?;
        // Per-node per-container 1-based instance count.
        let instance_id = node
            .container_instances
            .iter()
            .filter(|i| &i.container_id == container)
            .count()
            .saturating_add(1)
            .try_into()
            .unwrap_or(i32::MAX);
        let environment = node.environment.clone();
        node.container_instances.push(ContainerInstance {
            item,
            group: None,
            environment,
            container_id: container.clone(),
            instance_id,
            deployment_groups: clean_groups(deployment_groups),
            health_checks: Vec::new(),
            relationships: Vec::new(),
        });
        self.replicate_element_relationships(&id);
        Ok(id)
    }

    /// `deploymentGroup <deploymentGroups>` inside a node — Java
    /// `DeploymentNode.addDeploymentGroup`: blank names rejected, the name is
    /// added to the node's (deduplicated) set.
    pub fn add_deployment_group(&mut self, node: &Id, name: &str) -> Result<(), CoreError> {
        let node = self
            .node_mut(node)
            .ok_or_else(|| CoreError::DeploymentNodeNotFound { id: node.clone() })?;
        if name.trim().is_empty() {
            // The blank check trims.
            return Err(CoreError::DeploymentGroupNameRequired);
        }
        if !node.deployment_groups.iter().any(|g| g == name) {
            node.deployment_groups.push(name.to_string());
        }
        Ok(())
    }

    /// `healthCheck <name> <url> [interval] [timeout]`: validation and
    /// its messages match the official tooling; the check lands on the
    /// instance's list verbatim (name/url stored untrimmed).
    pub fn add_health_check(
        &mut self,
        instance: &Id,
        name: &str,
        url: &str,
        interval: i32,
        timeout: i64,
    ) -> Result<(), CoreError> {
        if name.trim().is_empty() {
            return Err(CoreError::HealthCheckInvalid(
                "The name must not be null or empty.".to_string(),
            ));
        }
        if url.trim().is_empty() {
            return Err(CoreError::HealthCheckInvalid(
                "The URL must not be null or empty.".to_string(),
            ));
        }
        if !crate::url::is_url(url) {
            return Err(CoreError::HealthCheckInvalid(format!(
                "{url} is not a valid URL."
            )));
        }
        if interval < 0 {
            return Err(CoreError::HealthCheckInvalid(
                "The polling interval must be zero or a positive integer.".to_string(),
            ));
        }
        if timeout < 0 {
            return Err(CoreError::HealthCheckInvalid(
                "The timeout must be zero or a positive integer.".to_string(),
            ));
        }
        let checks = Self::health_checks_mut(&mut self.workspace.model.deployment_nodes, instance)
            .ok_or_else(|| CoreError::InstanceNotFound {
                id: instance.clone(),
            })?;
        checks.push(crate::model::HttpHealthCheck {
            name: name.to_string(),
            url: url.to_string(),
            headers: std::collections::BTreeMap::new(),
            interval,
            timeout,
        });
        Ok(())
    }

    /// Mutable health-check list of a software-system / container instance.
    fn health_checks_mut<'a>(
        nodes: &'a mut [crate::model::DeploymentNode],
        id: &Id,
    ) -> Option<&'a mut Vec<crate::model::HttpHealthCheck>> {
        for n in nodes {
            for i in &mut n.software_system_instances {
                if &i.item.id == id {
                    return Some(&mut i.health_checks);
                }
            }
            for i in &mut n.container_instances {
                if &i.item.id == id {
                    return Some(&mut i.health_checks);
                }
            }
            if let Some(checks) = Self::health_checks_mut(&mut n.children, id) {
                return Some(checks);
            }
        }
        None
    }
}
