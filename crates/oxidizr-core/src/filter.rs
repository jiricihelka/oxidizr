//! Filtered-view evaluation:
//! [`eval`] decides which base-view content a [`crate::views::FilteredView`]
//! shows. The official tooling evaluates `mode` + `tags` CLIENT-SIDE
//! (the JSON only stores them — our writer matches, and this module
//! is deliberately NOT wired into it); semantics match the official
//! (include-on-diagram scope expansion, effective tag lookup, arrow
//! creation):
//!
//! * **Include** — keep an item iff ANY filter tag (each trimmed) hits its
//!   effective tag stack; an EMPTY tag list keeps NOTHING.
//! * **Exclude** — drop on ANY hit; an empty tag list keeps everything. An
//!   absent mode takes this branch too (the official evaluator tests
//!   for the literal `Include`; everything else behaves as exclude).
//! * Effective stacks: element = kind defaults + stored; deployment
//!   instances ALSO match the linked software-system/container stack; a
//!   relationship = its defaults + stored, and every `linkedRelationshipId`
//!   hop can supply the hit too. Filter tags are trimmed before comparing;
//!   item tags are matched verbatim.
//! * A relationship survives only when it passes its own filter AND both
//!   endpoint elements survive (arrows are drawn only between
//!   rendered boxes).
//!
//! The official tooling crashes on stale content ids (no guard); we
//! empty stack instead — an unresolvable element behaves like an untagged
//! one, an unresolvable relationship drops (the official tooling leaves
//! rendered at all there).
//!
//! Output preserves the input content order (determinism).

use std::collections::HashSet;

use crate::configuration::Workspace;
use crate::ids::Id;
use crate::model::Model;
use crate::tags::{TagKind, Tags};
use crate::validate::ElementKind;
use crate::views::{FilterMode, ViewContent};

/// What a filter keeps, in base-view content order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Included {
    /// Surviving element ids.
    pub element_ids: Vec<Id>,
    /// Surviving relationship ids.
    pub relationship_ids: Vec<Id>,
}

/// A model element resolved for tag matching: its own kind+stored tags, and
/// for deployment instances the static element whose tags ALSO count.
enum ElementRef<'m> {
    Plain {
        kind: ElementKind,
        tags: &'m Tags,
    },
    Instance {
        kind: ElementKind,
        tags: &'m Tags,
        linked: &'m Id,
    },
}

/// Evaluate `mode` + `tags` over `base` view content. `None` mode behaves
/// as [`FilterMode::Exclude`] (only the literal `Include` takes
/// the include branch).
#[must_use]
pub fn eval(
    workspace: &Workspace,
    base: &ViewContent,
    mode: Option<FilterMode>,
    tags: &[String],
) -> Included {
    let model = &workspace.model;
    let filter = |stack_hit: bool| match mode {
        Some(FilterMode::Include) => stack_hit,
        // Absent / unknown modes behave like Exclude.
        Some(FilterMode::Exclude) | None => !stack_hit,
    };

    let mut element_ids = Vec::with_capacity(base.elements.len());
    for element in &base.elements {
        let tags_hit = tags
            .iter()
            .any(|tag| element_matches(model, &element.id, tag.trim()));
        if filter(tags_hit) {
            element_ids.push(element.id.clone());
        }
    }
    let kept: HashSet<&Id> = element_ids.iter().collect();

    let mut relationship_ids = Vec::with_capacity(base.relationships.len());
    for relationship in &base.relationships {
        let Some(rel) = model.relationships.iter().find(|r| r.id == relationship.id) else {
            continue; // unresolvable relationship: nothing to route (see docs)
        };
        let tags_hit = tags.iter().any(|tag| {
            let tag = tag.trim();
            // Walk the linked-relationship chain;
            // a cycle stops at the first
            // revisit — the official evaluator would loop forever there.
            let mut seen: HashSet<&Id> = HashSet::new();
            let mut current = Some(rel);
            while let Some(r) = current {
                let defaults = crate::tags::relationship_defaults(
                    r.linked_relationship_id.is_some(),
                    r.interaction_style,
                );
                if r.tags.has_against(&defaults, tag) {
                    return true;
                }
                if !seen.insert(&r.id) {
                    break;
                }
                current = r
                    .linked_relationship_id
                    .as_ref()
                    .and_then(|lid| model.relationships.iter().find(|r| &r.id == lid));
            }
            false
        });
        if filter(tags_hit) && kept.contains(&rel.source) && kept.contains(&rel.destination) {
            relationship_ids.push(relationship.id.clone());
        }
    }

    Included {
        element_ids,
        relationship_ids,
    }
}

/// Whether `tag` hits the element's effective stack (instances: their own
/// AND the linked static element's).
fn element_matches(model: &Model, id: &Id, tag: &str) -> bool {
    let Some(found) = resolve(model, id) else {
        return false; // stale id: empty stack (see module docs)
    };
    let hits = |kind: ElementKind, tags: &Tags| tags.has(TagKind::from_element_kind(kind), tag);
    match found {
        ElementRef::Plain { kind, tags } => hits(kind, tags),
        ElementRef::Instance { kind, tags, linked } => {
            hits(kind, tags)
                // Instance-on-instance links do not exist; match is total anyway.
                || resolve(model, linked).is_some_and(|l| match l {
                    ElementRef::Plain { kind, tags }
                    | ElementRef::Instance { kind, tags, .. } => hits(kind, tags),
                })
        }
    }
}

/// First element with `id` in validation walk order, with its tag-matching bits.
fn resolve<'m>(model: &'m Model, id: &Id) -> Option<ElementRef<'m>> {
    use ElementRef::Plain;
    if let Some(ce) = model.custom_elements.iter().find(|ce| &ce.item.id == id) {
        return Some(Plain {
            kind: ElementKind::CustomElement,
            tags: &ce.item.tags,
        });
    }
    if let Some(p) = model.people.iter().find(|p| &p.item.id == id) {
        return Some(Plain {
            kind: ElementKind::Person,
            tags: &p.item.tags,
        });
    }
    for s in &model.software_systems {
        if &s.item.id == id {
            return Some(Plain {
                kind: ElementKind::SoftwareSystem,
                tags: &s.item.tags,
            });
        }
        for c in &s.containers {
            if &c.item.id == id {
                return Some(Plain {
                    kind: ElementKind::Container,
                    tags: &c.item.tags,
                });
            }
            for k in &c.components {
                if &k.item.id == id {
                    return Some(Plain {
                        kind: ElementKind::Component,
                        tags: &k.item.tags,
                    });
                }
            }
        }
    }
    resolve_node(&model.deployment_nodes, id)
}

fn resolve_node<'m>(nodes: &'m [crate::model::DeploymentNode], id: &Id) -> Option<ElementRef<'m>> {
    for n in nodes {
        if &n.item.id == id {
            return Some(ElementRef::Plain {
                kind: ElementKind::DeploymentNode,
                tags: &n.item.tags,
            });
        }
        for i in &n.infrastructure_nodes {
            if &i.item.id == id {
                return Some(ElementRef::Plain {
                    kind: ElementKind::InfrastructureNode,
                    tags: &i.item.tags,
                });
            }
        }
        for ssi in &n.software_system_instances {
            if &ssi.item.id == id {
                return Some(ElementRef::Instance {
                    kind: ElementKind::SoftwareSystemInstance,
                    tags: &ssi.item.tags,
                    linked: &ssi.software_system_id,
                });
            }
        }
        for ci in &n.container_instances {
            if &ci.item.id == id {
                return Some(ElementRef::Instance {
                    kind: ElementKind::ContainerInstance,
                    tags: &ci.item.tags,
                    linked: &ci.container_id,
                });
            }
        }
        if let Some(found) = resolve_node(&n.children, id) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::builder::WorkspaceBuilder;

    /// Alice (Live), Bank (Live), Legacy (tag "Legacy", no Live).
    fn fixture() -> WorkspaceBuilder {
        let mut b = WorkspaceBuilder::new();
        b.add_person("Alice", None, Some("Live")).unwrap();
        b.add_software_system("Bank", None, Some("Live")).unwrap();
        b.add_software_system("Legacy", Some("old".to_string()), Some("Legacy"))
            .unwrap();
        b
    }

    fn ids(ws: &WorkspaceBuilder) -> (Id, Id, Id) {
        let w = ws.workspace();
        let alice = w.model.people[0].item.id.clone();
        let bank = w.model.software_systems[0].item.id.clone();
        let legacy = w.model.software_systems[1].item.id.clone();
        (alice, bank, legacy)
    }

    fn content_of(element_ids: &[Id], relationship_ids: &[Id]) -> ViewContent {
        ViewContent {
            elements: element_ids
                .iter()
                .map(|id| crate::views::ElementView {
                    id: id.clone(),
                    x: 0,
                    y: 0,
                })
                .collect(),
            relationships: relationship_ids
                .iter()
                .map(|id| crate::views::RelationshipView {
                    id: id.clone(),
                    description: None,
                    url: None,
                    order: None,
                    response: None,
                    properties: crate::properties::Properties::new(),
                    vertices: Vec::new(),
                    routing: None,
                    jump: None,
                    position: None,
                })
                .collect(),
            ..ViewContent::default()
        }
    }

    fn tags(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|t| (*t).to_string()).collect()
    }

    #[test]
    fn include_keeps_any_tag_hits_in_content_order() {
        let b = fixture();
        let (alice, bank, legacy) = ids(&b);
        let content = content_of(&[legacy, bank.clone(), alice.clone()], &[]);
        let got = eval(
            b.workspace(),
            &content,
            Some(FilterMode::Include),
            &tags(&["Live"]),
        );
        assert_eq!(got.element_ids, vec![bank, alice]); // content order, not model order
    }

    #[test]
    fn include_with_empty_tags_keeps_nothing_exclude_keeps_everything() {
        let b = fixture();
        let (alice, bank, _legacy) = ids(&b);
        let content = content_of(&[bank.clone(), alice.clone()], &[]);
        let none = eval(
            b.workspace(),
            &content,
            Some(FilterMode::Include),
            &tags(&[]),
        );
        assert_eq!(none.element_ids.len(), 0);
        let all = eval(
            b.workspace(),
            &content,
            Some(FilterMode::Exclude),
            &tags(&[]),
        );
        assert_eq!(all.element_ids, vec![bank.clone(), alice.clone()]);
        // An absent mode behaves like Exclude.
        let absent = eval(b.workspace(), &content, None, &tags(&[]));
        assert_eq!(absent.element_ids, vec![bank, alice]);
    }

    #[test]
    fn exclude_drops_any_tag_hits_and_kind_defaults_match() {
        let b = fixture();
        let (alice, bank, legacy) = ids(&b);
        let content = content_of(&[alice, bank.clone(), legacy.clone()], &[]);
        // Kind default: every element carries "Element".
        let got = eval(
            b.workspace(),
            &content,
            Some(FilterMode::Exclude),
            &tags(&["Person"]),
        );
        assert_eq!(got.element_ids, vec![bank, legacy]);
    }

    #[test]
    fn filter_tags_trim_but_item_tags_match_verbatim() {
        let b = fixture();
        let (alice, bank, _legacy) = ids(&b);
        let content = content_of(&[bank.clone(), alice.clone()], &[]);
        let got = eval(
            b.workspace(),
            &content,
            Some(FilterMode::Include),
            &tags(&["  Live  "]),
        );
        assert_eq!(got.element_ids, vec![bank, alice]); // both carry Live; trimmed hit
    }

    #[test]
    fn relationship_needs_its_own_hit_and_both_endpoints_surviving() {
        let mut b = fixture();
        let (alice, bank, legacy) = {
            let w = b.workspace();
            (
                w.model.people[0].item.id.clone(),
                w.model.software_systems[0].item.id.clone(),
                w.model.software_systems[1].item.id.clone(),
            )
        };
        b.add_relationship(&alice, &bank, None, None, None).unwrap();
        b.add_relationship(&alice, &legacy, None, None, None)
            .unwrap();
        let rel_ids: Vec<Id> = b
            .workspace()
            .model
            .relationships
            .iter()
            .map(|r| r.id.clone())
            .collect();
        let content = content_of(&[alice.clone(), bank.clone(), legacy.clone()], &rel_ids);

        // No relationship tags -> Include keeps neither relationship even
        // though both endpoints survived.
        let got = eval(
            b.workspace(),
            &content,
            Some(FilterMode::Include),
            &tags(&["Live"]),
        );
        assert_eq!(got.element_ids, vec![alice.clone(), bank.clone()]); // content order
        assert_eq!(got.relationship_ids.len(), 0);

        // Unfiltered (Exclude, empty) keeps both.
        let got = eval(
            b.workspace(),
            &content,
            Some(FilterMode::Exclude),
            &tags(&[]),
        );
        assert_eq!(got.relationship_ids, rel_ids);

        // Endpoint rule: excluding the target drops the relationship to it.
        let got = eval(
            b.workspace(),
            &content,
            Some(FilterMode::Exclude),
            &tags(&["Legacy"]),
        );
        assert_eq!(got.relationship_ids, rel_ids[..1]);
    }

    #[test]
    fn deployment_instance_matches_its_static_element_stack() {
        let mut b = fixture();
        let bank = {
            let w = b.workspace();
            w.model.software_systems[0].item.id.clone()
        };
        let node = b
            .add_deployment_node(crate::DEFAULT_ENVIRONMENT, "AWS", None, None, None)
            .unwrap();
        let ssi = b
            .add_software_system_instance(&node, &bank, None, Vec::new())
            .unwrap();
        // "Live" lives on the Bank system; the instance must match it.
        let content = content_of(std::slice::from_ref(&ssi), &[]);
        let got = eval(
            b.workspace(),
            &content,
            Some(FilterMode::Include),
            &tags(&["Live"]),
        );
        assert_eq!(got.element_ids, vec![ssi.clone()]);
        // Its own default must match too.
        let got = eval(
            b.workspace(),
            &content,
            Some(FilterMode::Include),
            &tags(&["Software System Instance"]),
        );
        assert_eq!(got.element_ids, vec![ssi]);
    }

    #[test]
    fn stale_ids_resolve_to_empty_stack() {
        let b = fixture();
        let ghost = Id::new("9999");
        let content = content_of(std::slice::from_ref(&ghost), std::slice::from_ref(&ghost));
        let all = eval(
            b.workspace(),
            &content,
            Some(FilterMode::Exclude),
            &tags(&[]),
        );
        // Element: empty stack survives Exclude; relationship: unresolvable drops.
        assert_eq!(all.element_ids, vec![ghost]);
        assert_eq!(all.relationship_ids.len(), 0);
        let inc = eval(
            b.workspace(),
            &content,
            Some(FilterMode::Include),
            &tags(&["x"]),
        );
        assert_eq!(inc.element_ids.len(), 0);
    }

    #[test]
    fn evaluation_is_deterministic() {
        let mut b = fixture();
        let w = b.workspace();
        let (alice, bank) = (
            w.model.people[0].item.id.clone(),
            w.model.software_systems[0].item.id.clone(),
        );
        b.add_relationship(&alice, &bank, None, None, None).unwrap();
        let rel = b.workspace().model.relationships[0].id.clone();
        let content = content_of(&[alice, bank], &[rel]);
        let first = eval(
            b.workspace(),
            &content,
            Some(FilterMode::Exclude),
            &tags(&["Person"]),
        );
        let second = eval(
            b.workspace(),
            &content,
            Some(FilterMode::Exclude),
            &tags(&["Person"]),
        );
        assert_eq!(first, second);
    }
}
