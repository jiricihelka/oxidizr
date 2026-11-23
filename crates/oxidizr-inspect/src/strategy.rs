//! The property-based severity strategy: every run site resolves one
//! violation's severity from the inspection TYPE and a chain of property
//! HOLDERS, searched inner→outer, first key hit wins:
//!
//! * element     ⇒ element → parent → grandparent → model → workspace
//! * relationship⇒ relationship → linked → source → source.parent → model
//!   → workspace (with the `[srcType->dstType]` type rewrite,
//!   specific key before generic)
//! * view        ⇒ view → view-config → workspace
//! * element style ⇒ style → view-config → workspace
//! * model       ⇒ model → workspace
//! * views / workspace ⇒ views-config → workspace / workspace
//!
//! No hit anywhere ⇒ [`Severity::Error`] — so ALL inspections default to
//! error.

use oxidizr_core::properties::Properties;

use crate::severity::Severity;

/// The property prefix — note the SINGULAR `inspection`.
pub const PREFIX: &str = "structurizr.inspection.";

/// The kill switch: `structurizr.inspection` equal to `"false"` under
/// ANY case skips the whole run.
#[must_use]
pub fn enabled(workspace_properties: &Properties) -> bool {
    workspace_properties
        .get("structurizr.inspection")
        .is_none_or(|v| !v.eq_ignore_ascii_case("false"))
}

/// The wildcard candidate chain for a type, longest-first. Example for
/// `model.component.description`:
///
/// ```text
/// structurizr.inspection.model.component.description
/// structurizr.inspection.model.component.*
/// structurizr.inspection.model.*
/// structurizr.inspection.*
/// ```
///
/// For `.relationship[…].`-scoped types the last two (generic) fallbacks
/// are DROPPED: a relationship key can only be set per
/// source→destination pair or with its own exact key.
#[must_use]
pub fn property_names(type_path: &str) -> Vec<String> {
    let lowered = type_path.to_lowercase();
    let mut names = vec![format!("{PREFIX}*")];
    let mut buf = String::from(PREFIX);
    let parts: Vec<&str> = lowered.split('.').collect();
    for part in &parts[..parts.len().saturating_sub(1)] {
        buf.push_str(part);
        buf.push('.');
        names.push(format!("{buf}*"));
    }
    names.push(format!("{PREFIX}{lowered}"));
    names.reverse();
    if lowered.contains(".relationship[") {
        // Catalog types always have >= 4 candidates; the saturating
        // guard covers hand-written inputs.
        names.truncate(names.len().saturating_sub(2));
    }
    names
}

/// The first holder (INNERMOST first) containing one candidate key decides;
/// its value parses via [`Severity::parse`]. `None` = no candidate key in
/// any holder.
#[must_use]
pub fn severity_lookup(type_path: &str, holders: &[Option<&Properties>]) -> Option<Severity> {
    let names = property_names(type_path);
    for holder in holders.iter().flatten() {
        for name in &names {
            if let Some(value) = holder.get(name) {
                return Some(Severity::parse(value));
            }
        }
    }
    None
}

/// The non-relationship entry point: lookup, else the ERROR default.
#[must_use]
pub fn severity_from(type_path: &str, holders: &[Option<&Properties>]) -> Severity {
    severity_lookup(type_path, holders).unwrap_or(Severity::Error)
}

/// The lowercase element-kind token used in the `[srcType->dstType]`
/// key rewrite.
#[must_use]
pub fn java_element_type(element_kind: &str) -> String {
    // `element_kind` is already the canonical lowercase kind token
    // ("softwaresystem", "deploymentnode", …), so this is the identity
    // map.
    element_kind.to_owned()
}

/// The relationship severity resolution: the `.relationship.` segment
/// rewritten to `.relationship[<src>-><dst>].` is tried FIRST, the
/// generic type SECOND; the holders are searched inner→outer in both
/// passes.
#[must_use]
pub fn relationship_severity(
    type_path: &str,
    source_kind: &str,
    destination_kind: &str,
    holders: &[Option<&Properties>],
) -> Severity {
    // Only the FIRST `.relationship.` occurrence is rewritten.
    if let Some(at) = type_path.find(".relationship.") {
        let specific = format!(
            "{}.relationship[{}->{}].{}",
            &type_path[..at],
            java_element_type(source_kind),
            java_element_type(destination_kind),
            &type_path[at + ".relationship.".len()..]
        );
        if let Some(sev) = severity_lookup(&specific, holders) {
            return sev;
        }
    }
    severity_from(type_path, holders)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(pairs: &[(&str, &str)]) -> Properties {
        let mut p = Properties::new();
        for (k, v) in pairs {
            p.insert(*k, *v);
        }
        p
    }

    #[test]
    fn chain_generation_element_style() {
        assert_eq!(
            property_names("model.component.description"),
            [
                "structurizr.inspection.model.component.description",
                "structurizr.inspection.model.component.*",
                "structurizr.inspection.model.*",
                "structurizr.inspection.*",
            ]
        );
        // Types are lowercased before splitting.
        assert_eq!(
            property_names("Views.View.Empty")[0],
            "structurizr.inspection.views.view.empty"
        );
    }

    #[test]
    fn chain_generation_relationship_drops_generic_fallbacks() {
        // Relationship-scoped chains keep no `…model.*` /
        // `…inspection.*` leg.
        assert_eq!(
            property_names("model.relationship[person->softwaresystem].description"),
            [
                "structurizr.inspection.model.relationship[person->softwaresystem].description",
                "structurizr.inspection.model.relationship[person->softwaresystem].*",
            ]
        );
    }

    #[test]
    fn innermost_holder_wins_and_nothing_matches_is_error() {
        let outer = props(&[("structurizr.inspection.model.person.description", "info")]);
        let inner = props(&[("structurizr.inspection.model.person.description", "warning")]);
        assert_eq!(
            severity_from(
                "model.person.description",
                &[Some(&inner), Some(&outer), None]
            ),
            Severity::Warning
        );
        // Innermost WITHOUT a matching key falls through to the outer
        // holder (its wildcard leg is a candidate; a key for ANOTHER type
        // never matches — candidates are exact per type path).
        assert_eq!(
            severity_from(
                "model.softwaresystem.description",
                &[Some(&inner), Some(&outer)]
            ),
            Severity::Error
        );
        let outer_wild = props(&[("structurizr.inspection.model.*", "info")]);
        assert_eq!(
            severity_from(
                "model.softwaresystem.description",
                &[Some(&inner), Some(&outer_wild)]
            ),
            Severity::Info
        );
        assert_eq!(severity_from("model.empty", &[None, None]), Severity::Error);
    }

    #[test]
    fn wildcard_legs_are_candidates() {
        let p = props(&[("structurizr.inspection.model.*", "ignore")]);
        assert_eq!(
            severity_from("model.component.technology", &[Some(&p)]),
            Severity::Ignore
        );
    }

    #[test]
    fn relationship_specificity() {
        // A `[person->softwaresystem]`-specific key does NOT fire on a
        // container->container relationship (and the generic wildcard is
        // not consulted for relationship-scoped lookups at all).
        let p = props(&[(
            "structurizr.inspection.model.relationship[person->softwaresystem].description",
            "warning",
        )]);
        assert_eq!(
            relationship_severity(
                "model.relationship.description",
                "person",
                "softwaresystem",
                &[Some(&p)]
            ),
            Severity::Warning
        );
        assert_eq!(
            relationship_severity(
                "model.relationship.description",
                "container",
                "container",
                &[Some(&p)]
            ),
            Severity::Error
        );
    }

    #[test]
    fn relationship_generic_fallback_after_specific_miss() {
        let p = props(&[(
            "structurizr.inspection.model.relationship.description",
            "info",
        )]);
        assert_eq!(
            relationship_severity(
                "model.relationship.technology",
                "person",
                "softwaresystem",
                &[Some(&p)]
            ),
            // Generic type `model.relationship.technology` misses…
            Severity::Error
        );
        assert_eq!(
            relationship_severity(
                "model.relationship.description",
                "person",
                "softwaresystem",
                &[Some(&p)]
            ),
            Severity::Info
        );
    }

    #[test]
    fn kill_switch() {
        assert!(!enabled(&props(&[("structurizr.inspection", "False")])));
        assert!(enabled(&props(&[("structurizr.inspection", "true")])));
        assert!(enabled(&props(&[])));
    }
}
