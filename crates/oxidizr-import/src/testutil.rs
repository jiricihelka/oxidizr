//! Shared test helpers for the macro importers' goldens (compact model
//! digest: elements nested under their parents, relationships,
//! warnings). Not compiled into the shipped library.

#![allow(clippy::format_push_string)] // test digest: format! reads best here

use std::collections::HashMap;

use oxidizr_core::Workspace;
use oxidizr_core::tags::TagKind;

use crate::Imported;

/// id → display name (containers/components path-qualified).
pub fn names(ws: &Workspace) -> HashMap<&str, String> {
    let mut map = HashMap::new();
    for p in &ws.model.people {
        map.insert(p.item.id.as_str(), p.item.name.clone());
    }
    for s in &ws.model.software_systems {
        map.insert(s.item.id.as_str(), s.item.name.clone());
        for c in &s.containers {
            map.insert(
                c.item.id.as_str(),
                format!("{} / {}", s.item.name, c.item.name),
            );
            for k in &c.components {
                map.insert(
                    k.item.id.as_str(),
                    format!("{} / {} / {}", s.item.name, c.item.name, k.item.name),
                );
            }
        }
    }
    map
}

/// Compact model digest for the `expect!` goldens.
pub fn digest(im: &Imported) -> String {
    let ws = &im.workspace;
    let n = names(ws);
    let mut out = String::new();
    for p in &ws.model.people {
        out.push_str(&format!(
            "person {} [{}] {:?}\n",
            p.item.name,
            p.item.tags.serialized_for(TagKind::Person),
            p.item.description
        ));
    }
    for s in &ws.model.software_systems {
        out.push_str(&format!(
            "system {} [{}] {:?}\n",
            s.item.name,
            s.item.tags.serialized_for(TagKind::SoftwareSystem),
            s.item.description
        ));
        for c in &s.containers {
            out.push_str(&format!(
                "  container {} [{}] tech={:?} {:?}\n",
                c.item.name,
                c.item.tags.serialized_for(TagKind::Container),
                c.technology,
                c.item.description
            ));
            for k in &c.components {
                out.push_str(&format!(
                    "    component {} [{}] tech={:?}\n",
                    k.item.name,
                    k.item.tags.serialized_for(TagKind::Component),
                    k.technology
                ));
            }
        }
    }
    for r in &ws.model.relationships {
        out.push_str(&format!(
            "rel {} -> {} \"{}\" tech={:?} [{}]\n",
            n[r.source.as_str()],
            n[r.destination.as_str()],
            r.description.clone().unwrap_or_default(),
            r.technology,
            r.tags.stored().join("+")
        ));
    }
    for w in &im.warnings {
        out.push_str(&format!("warn: {w}\n"));
    }
    out
}
