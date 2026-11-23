//! Canonical names — the v6 URI-style scheme (the v5-era
//! `Person[X]` rendering is long gone).
//!
//! Every name part passes through `formatName`: the static separator `.` and the
//! deployment separator `/` are REMOVED from each part before joining, so
//! `Container://Sys.C` for system "S.y" + container "C" comes from "Sy"+"C".
//! The deployment chain drops the `DeploymentNode://` prefix when an
//! infrastructure node / instance appends to it.

/// `formatName(String)`: strip `.` and `/` from a name part.
#[must_use]
pub fn format_name(name: &str) -> String {
    name.replace(['.', '/'], "")
}

/// `Person://X`
#[must_use]
pub fn person(name: &str) -> String {
    format!("Person://{}", format_name(name))
}

/// `SoftwareSystem://S`
#[must_use]
pub fn software_system(name: &str) -> String {
    format!("SoftwareSystem://{}", format_name(name))
}

/// `Custom://X`
#[must_use]
pub fn custom_element(name: &str) -> String {
    format!("Custom://{}", format_name(name))
}

/// `Container://S.C`
#[must_use]
pub fn container(system: &str, name: &str) -> String {
    format!("Container://{}.{}", format_name(system), format_name(name))
}

/// `Component://S.C.K`
#[must_use]
pub fn component(system: &str, container: &str, name: &str) -> String {
    format!(
        "Component://{}.{}.{}",
        format_name(system),
        format_name(container),
        format_name(name)
    )
}

/// `DeploymentNode://ENV/A/B` — `chain` is the ancestor node names, root first
/// (already-formatted names accepted; they are re-`formatName`d, idempotently).
#[must_use]
pub fn deployment_node(environment: &str, chain: &[&str], name: &str) -> String {
    let mut out = format!("DeploymentNode://{}", format_name(environment));
    for part in chain.iter().chain(std::iter::once(&name)) {
        out.push('/');
        out.push_str(&format_name(part));
    }
    out
}

/// The deployment-node chain WITHOUT its scheme prefix, reused by
/// infrastructure nodes and instances: the
/// chain WITHOUT its scheme prefix.
#[must_use]
pub fn deployment_node_chain(environment: &str, chain: &[&str]) -> String {
    deployment_node(environment, chain, "")
        .trim_start_matches("DeploymentNode://")
        .trim_end_matches('/')
        .to_string()
}

/// `InfrastructureNode://ENV/A/B/N`
#[must_use]
pub fn infrastructure_node(node_chain_without_scheme: &str, name: &str) -> String {
    format!(
        "InfrastructureNode://{}/{}",
        node_chain_without_scheme,
        format_name(name)
    )
}

/// `SoftwareSystemInstance://ENV/A/B/System[instanceId]`
#[must_use]
pub fn software_system_instance(
    node_chain_without_scheme: &str,
    system_name: &str,
    instance_id: i32,
) -> String {
    format!(
        "SoftwareSystemInstance://{}/{}[{}]",
        node_chain_without_scheme,
        format_name(system_name),
        instance_id
    )
}

/// `ContainerInstance://ENV/A/B/Sys.Container[instanceId]`
#[must_use]
pub fn container_instance(
    node_chain_without_scheme: &str,
    system_name: &str,
    container_name: &str,
    instance_id: i32,
) -> String {
    format!(
        "ContainerInstance://{}/{}.{}[{}]",
        node_chain_without_scheme,
        format_name(system_name),
        format_name(container_name),
        instance_id
    )
}

/// `Relationship://src -> dst (description)` (no parens part when the
/// description is null/empty).
#[must_use]
pub fn relationship(
    source_canonical: &str,
    destination_canonical: &str,
    description: Option<&str>,
) -> String {
    match description {
        Some(d) if !d.is_empty() => {
            format!("Relationship://{source_canonical} -> {destination_canonical} ({d})")
        }
        _ => format!("Relationship://{source_canonical} -> {destination_canonical}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_name_strips_separators() {
        assert_eq!(format_name("a.b/c"), "abc");
    }

    #[test]
    fn static_structures() {
        assert_eq!(person("A. User"), "Person://A User");
        assert_eq!(
            software_system("Internet Banking"),
            "SoftwareSystem://Internet Banking"
        );
        assert_eq!(
            container("Internet Banking", ".NET App"),
            "Container://Internet Banking.NET App"
        );
        assert_eq!(
            component("Bank", "Web", "Controller"),
            "Component://Bank.Web.Controller"
        );
        assert_eq!(
            custom_element("Kubernetes Cluster"),
            "Custom://Kubernetes Cluster"
        );
    }

    #[test]
    fn deployment_chain() {
        assert_eq!(
            deployment_node("Live", &["us-east-1", "backend"], "VM 1"),
            "DeploymentNode://Live/us-east-1/backend/VM 1"
        );
        assert_eq!(
            deployment_node_chain("Live", &["us-east-1"]),
            "Live/us-east-1"
        );
        assert_eq!(
            infrastructure_node("Live/us-east-1", "ALB"),
            "InfrastructureNode://Live/us-east-1/ALB"
        );
        assert_eq!(
            software_system_instance("Live/us-east-1", "Bank", 5),
            "SoftwareSystemInstance://Live/us-east-1/Bank[5]"
        );
        assert_eq!(
            container_instance("Live/us-east-1", "Bank", "Web", 7),
            "ContainerInstance://Live/us-east-1/Bank.Web[7]"
        );
    }

    #[test]
    fn relationship_name() {
        assert_eq!(
            relationship("Person://U", "SoftwareSystem://S", Some("Uses")),
            "Relationship://Person://U -> SoftwareSystem://S (Uses)"
        );
        assert_eq!(
            relationship("Person://U", "SoftwareSystem://S", Some("")),
            "Relationship://Person://U -> SoftwareSystem://S"
        );
    }
}
