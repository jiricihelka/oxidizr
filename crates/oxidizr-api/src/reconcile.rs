//! `reconcile` — re-attach saved layout geometry to a re-parsed DSL workspace
//! (an additional feature).
//!
//! Problem: the shared sequential id counter regenerates ids on every
//! parse, so reordering or inserting DSL statements shifts ids and silently
//! detaches the saved `{id,x,y}` layout geometry in a layout JSON produced
//! from the pre-edit workspace.
//!
//! Algorithm (in order):
//! 1. old→new id maps for elements and relationships, built from the LAYOUT
//!    workspace (old ids) against the re-parsed DSL workspace (new ids):
//!    exact `structurizr.dsl.identifier` match first, then the canonical-name
//!    fallback ([`WorkspaceBuilder::canonical_name_full`] /
//!    [`WorkspaceBuilder::relationship_canonical_name`] over `adopt`ed
//!    clones). Anonymous elements carry volatile-uuid identifiers, so they
//!    never hit the identifier path and fall through to the name match —
//!    exactly the intent.
//! 2. A lookup with more than one candidate, or two old ids competing for one
//!    new id, is AMBIGUOUS: an OXZ-3002 issue, never a guess. Unmatched old
//!    ids simply drop out (reported only where a view referenced them).
//! 3. The re-parsed workspace is the output — the DSL decides the model and
//!    view membership (the user's latest edit), the layout decides geometry:
//!    element x/y, relationship-view routing fields, view-level
//!    paper/dimensions/automaticLayout and (id-remapped) animation steps are
//!    grafted through the maps. Descriptions, order/response and view
//!    metadata stay with the DSL.
//! 4. The human report (remapped / dropped / ambiguous) is rendered by the
//!    CLI; see [`reconcile`] and [`Report`].

use std::collections::{HashMap, HashSet};

use oxidizr_core::Workspace;
use oxidizr_core::builder::WorkspaceBuilder;
use oxidizr_core::ids::Id;
use oxidizr_core::validate::flattened;
use oxidizr_core::views::{Animation, View, ViewContent, ViewSet};
use oxidizr_diag::codes;

use crate::Issue;

/// The DSL-stamped identity property (a locked compat decision).
const DSL_IDENTIFIER: &str = "structurizr.dsl.identifier";

/// One workspace's lookup tables: identifier value → ids, canonical → ids.
#[derive(Default)]
struct Index {
    by_ident: HashMap<String, Vec<Id>>,
    by_canonical: HashMap<String, Vec<Id>>,
}

impl Index {
    /// Build both tables for the RELATIONSHIPS of `ws` (the
    /// `rels: bool` switch split into the two named builders it dispatches
    /// on — call sites pick the id space by name, never by flag).
    fn build_relationships(ws: &Workspace, b: &WorkspaceBuilder) -> Index {
        let mut idx = Index::default();
        for r in &ws.model.relationships {
            if let Some(ident) = r.properties.get(DSL_IDENTIFIER) {
                idx.by_ident
                    .entry(ident.to_string())
                    .or_default()
                    .push(r.id.clone());
            }
            if let Some(cn) = b.relationship_canonical_name(&r.id) {
                idx.by_canonical.entry(cn).or_default().push(r.id.clone());
            }
        }
        idx
    }

    /// [`Self::build_relationships`] for the flattened-model ELEMENTS of
    /// `ws` (split).
    fn build_elements(ws: &Workspace, b: &WorkspaceBuilder) -> Index {
        let mut idx = Index::default();
        for (id, _, _) in flattened(&ws.model) {
            if let Some(ident) = b.item(&id).and_then(|i| i.properties.get(DSL_IDENTIFIER)) {
                idx.by_ident
                    .entry(ident.to_string())
                    .or_default()
                    .push(id.clone());
            }
            if let Some(cn) = b.canonical_name_full(&id) {
                idx.by_canonical.entry(cn).or_default().push(id.clone());
            }
        }
        idx
    }
}

/// old→new id remapping plus everything that went wrong along the way.
#[derive(Default)]
struct Maps {
    /// old id → new id (single candidate, uncontested).
    to_new: HashMap<Id, Id>,
    /// new id → old id (the graft lookup).
    to_old: HashMap<Id, Id>,
    /// new ids whose claim was contested — later claims for them are refused
    /// too (the first claimant was revoked, "never guessed" means NEITHER).
    poisoned: std::collections::HashSet<Id>,
    /// OXZ-3002: >1 candidate, or two old ids claiming one new id.
    ambiguous: Vec<Issue>,
}

impl Maps {
    fn claim(&mut self, old: &Id, new: &Id, what: &str, why: &str) {
        if let Some(contender) = self.to_old.remove(new) {
            // Two old ids compete for one new id — revoke the first claim,
            // neither is remapped, the new id stays poisoned.
            self.to_new.remove(&contender);
            self.poisoned.insert(new.clone());
            self.ambiguous.push(Issue::error(
                codes::RECONCILE,
                format!(
                    "ambiguous reconcile: {what} `{contender}` and `{old}` ({why}) both resolve to new id `{new}` — neither remapped"
                ),
            ));
            return;
        }
        if self.poisoned.contains(new) {
            self.ambiguous.push(Issue::error(
                codes::RECONCILE,
                format!(
                    "ambiguous reconcile: {what} `{old}` ({why}) also resolves to contested new id `{new}` — not remapped"
                ),
            ));
            return;
        }
        self.to_new.insert(old.clone(), new.clone());
        self.to_old.insert(new.clone(), old.clone());
    }

    fn ambiguous_hit(&mut self, old: &Id, what: &str, why: &str, candidates: &[Id]) {
        let list: Vec<String> = candidates.iter().map(ToString::to_string).collect();
        self.ambiguous.push(Issue::error(
            codes::RECONCILE,
            format!(
                "ambiguous reconcile: {what} `{old}` ({why}) matches {} new candidates ({}) — not remapped",
                candidates.len(),
                list.join(", ")
            ),
        ));
    }
}

type OldIds = Vec<(Id, Option<String>, Option<String>)>;

/// The old-side relationship rows: id + identifier value + canonical name
/// (the `rels: bool` branch of `map_ids` became these two
/// named collectors — the id space is chosen by name at the call site).
fn old_relationship_ids(old_ws: &Workspace, old_b: &WorkspaceBuilder) -> OldIds {
    old_ws
        .model
        .relationships
        .iter()
        .map(|r| {
            (
                r.id.clone(),
                r.properties.get(DSL_IDENTIFIER).map(str::to_string),
                old_b.relationship_canonical_name(&r.id),
            )
        })
        .collect()
}

/// [`old_relationship_ids`] for the flattened-model elements .
fn old_element_ids(old_ws: &Workspace, old_b: &WorkspaceBuilder) -> OldIds {
    flattened(&old_ws.model)
        .into_iter()
        .map(|(id, _, _)| {
            let ident = old_b
                .item(&id)
                .and_then(|i| i.properties.get(DSL_IDENTIFIER))
                .map(str::to_string);
            let canonical = old_b.canonical_name_full(&id);
            (id, ident, canonical)
        })
        .collect()
}

/// Build the old→new maps for one id space, given its old-side rows.
///
/// Matching per old id: exact `structurizr.dsl.identifier` match first;
/// then the canonical-name fallback (identifier absent or renamed away).
fn map_ids(olds: OldIds, new_idx: &Index, what: &str) -> Maps {
    let mut maps = Maps::default();
    for (old, ident, canonical) in olds {
        // (1) exact `structurizr.dsl.identifier` match.
        if let Some(ident) = ident.as_deref()
            && let Some(cands) = new_idx.by_ident.get(ident)
        {
            match cands.as_slice() {
                [one] => maps.claim(&old, one, what, &format!("identifier `{ident}`")),
                many => maps.ambiguous_hit(&old, what, &format!("identifier `{ident}`"), many),
            }
            continue;
        }
        // (2) canonical-name fallback (identifier absent, or renamed away).
        if let Some(cn) = canonical.as_deref()
            && let Some(cands) = new_idx.by_canonical.get(cn)
        {
            match cands.as_slice() {
                [one] => maps.claim(&old, one, what, &format!("canonical name `{cn}`")),
                many => maps.ambiguous_hit(&old, what, &format!("canonical name `{cn}`"), many),
            }
        }
        // (3) no match: dropped (reported only when a view referenced it).
    }
    maps
}

/// Geometry source for one layout view.
struct OldView<'a> {
    content: Option<&'a ViewContent>,
    animations: Option<&'a Vec<Animation>>,
}

fn old_view<'b>(v: &'b View<'_>) -> OldView<'b> {
    OldView {
        content: v.content(),
        animations: match v {
            View::Landscape(v) => Some(&v.animations),
            View::Context(v) => Some(&v.animations),
            View::Container(v) => Some(&v.animations),
            View::Component(v) => Some(&v.animations),
            View::Deployment(v) => Some(&v.animations),
            View::Custom(v) => Some(&v.animations),
            View::Dynamic(_) | View::Filtered(_) | View::Image(_) => None,
        },
    }
}

/// What the graft did — the human report the CLI renders.
#[derive(Default)]
pub struct Report {
    /// Views that received layout.
    pub views: usize,
    /// Element-view x/y grafts.
    pub elements: usize,
    /// Relationship-view grafts.
    pub relationships: usize,
    /// EVERY uncontested old→new ELEMENT id pair the remap established
    /// (sorted by old id for a deterministic order). The CLI report
    /// never prints these; the viewer's reload path uses them to re-key
    /// its session overlays (§). Ambiguous ids appear in NEITHER
    /// this list NOR `ambiguous`-carried guesses — never guessed means
    /// never here. Elements and relationships keep SEPARATE lists:
    /// both id spaces mint `"1"`, `"2"`, … independently, so a merged
    /// pair list would be ambiguous by construction.
    pub remapped_elements: Vec<(Id, Id)>,
    /// The relationship counterpart of [`Report::remapped_elements`].
    pub remapped_relationships: Vec<(Id, Id)>,
    /// Layout entries that could not be placed (the DSL edit removed or
    /// excluded them — expected outcomes, reported for review).
    pub dropped: Vec<String>,
    /// OXZ-3002 issues: ambiguous, never guessed (exit 1).
    pub ambiguous: Vec<Issue>,
}

impl Report {
    /// One-line summary for the human report.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "reconciled {} view(s): {} element(s), {} relationship(s) remapped",
            self.views, self.elements, self.relationships
        )
    }
}

/// Re-attach `layout` geometry onto `updated` (the re-parsed DSL workspace).
#[must_use]
pub fn reconcile(layout: &Workspace, updated: &Workspace) -> (Workspace, Report) {
    let mut report = Report::default();

    // Canonical-name machinery over adopted clones (: reconcile is pure
    // composition — no core changes).
    let mut new_b = WorkspaceBuilder::new();
    new_b.adopt(updated.clone());
    let mut old_b = WorkspaceBuilder::new();
    old_b.adopt(layout.clone());

    let mut elems = map_ids(
        old_element_ids(layout, &old_b),
        &Index::build_elements(updated, &new_b),
        "element",
    );
    let mut rels = map_ids(
        old_relationship_ids(layout, &old_b),
        &Index::build_relationships(updated, &new_b),
        "relationship",
    );
    report.ambiguous.append(&mut elems.ambiguous);
    report.ambiguous.append(&mut rels.ambiguous);
    // The full uncontested mappings, deterministic order (the viewer's
    // overlay re-key; §). Collected BEFORE grafting, sorted per
    // kind so runs agree and the overlapping id spaces never mix.
    let sorted = |maps: &Maps| {
        let mut pairs: Vec<(Id, Id)> = maps
            .to_new
            .iter()
            .map(|(old, new)| (old.clone(), new.clone()))
            .collect();
        pairs.sort();
        pairs
    };
    report.remapped_elements = sorted(&elems);
    report.remapped_relationships = sorted(&rels);

    let old_views = layout.views.all_views();
    let old_by_key: HashMap<&str, OldView<'_>> =
        old_views.iter().map(|v| (v.key(), old_view(v))).collect();

    let mut out = updated.clone();
    graft_set(&mut out.views, &old_by_key, &elems, &rels, &mut report);
    (out, report)
}

/// Graft every view of one `ViewSet` onto its layout counterpart (key match).
fn graft_set(
    views: &mut ViewSet,
    old_by_key: &HashMap<&str, OldView<'_>>,
    elems: &Maps,
    rels: &Maps,
    report: &mut Report,
) {
    macro_rules! graft_kind {
        ($vec:expr) => {
            for v in &mut $vec {
                let Some(old) = old_by_key.get(v.base.key.as_str()) else {
                    continue;
                };
                report.views += 1;
                if let Some(oldc) = old.content {
                    graft_content(&mut v.content, oldc, &v.base.key, elems, rels, report);
                    // View-level geometry: the layout decides (the DSL
                    // cannot express any of it).
                    oldc.paper_size.clone_into(&mut v.content.paper_size);
                    oldc.dimensions.clone_into(&mut v.content.dimensions);
                    if v.content.automatic_layout.is_none() {
                        v.content.automatic_layout = oldc.automatic_layout;
                    }
                }
            }
        };
    }
    graft_kind!(views.custom_views);
    graft_kind!(views.system_landscape_views);
    graft_kind!(views.system_context_views);
    graft_kind!(views.container_views);
    graft_kind!(views.component_views);
    graft_kind!(views.dynamic_views);
    graft_kind!(views.deployment_views);

    // Animations: the layout steps, remapped onto new ids, replace the
    // re-parsed view's steps (the reveal order is layout content).
    macro_rules! anim_kind {
        ($vec:expr) => {
            for v in &mut $vec {
                let Some(old) = old_by_key.get(v.base.key.as_str()) else {
                    continue;
                };
                if let Some(olda) = old.animations {
                    v.animations = remap_animations(olda, &v.base.key, elems, rels, report);
                }
            }
        };
    }
    anim_kind!(views.custom_views);
    anim_kind!(views.system_landscape_views);
    anim_kind!(views.system_context_views);
    anim_kind!(views.container_views);
    anim_kind!(views.component_views);
    anim_kind!(views.deployment_views);

    // Layout views with no counterpart in the re-parsed workspace
    // (`HashSet` membership replaces the per-key `Vec::contains`
    // scan — same answer, no quadratic re-scan).
    let new_keys: HashSet<&str> = views.all_views().into_iter().map(View::key).collect();
    for key in old_by_key.keys() {
        if !new_keys.contains(key) {
            report
                .dropped
                .push(format!("view \"{key}\" — not in the re-parsed workspace"));
        }
    }
}

/// Remap one animation's string id lists; steps left empty are dropped.
fn remap_animations(
    olda: &[Animation],
    view_key: &str,
    elems: &Maps,
    rels: &Maps,
    report: &mut Report,
) -> Vec<Animation> {
    // the closure shadows the module-level `map_ids` fn — `remap_ids`.
    let mut remap_ids = |ids: &[String], map: &Maps, what: &str, order: i32| -> Vec<String> {
        ids.iter()
            .filter_map(|id| {
                if let Some(new) = map.to_new.get(&Id::new(id.as_str())) {
                    Some(new.to_string())
                } else {
                    report.dropped.push(format!(
                        "view \"{view_key}\" animation step {order}: {what} id `{id}` is gone"
                    ));
                    None
                }
            })
            .collect()
    };
    olda.iter()
        .filter_map(|a| {
            let step = Animation {
                order: a.order,
                element_ids: remap_ids(&a.element_ids, elems, "element", a.order),
                relationship_ids: remap_ids(&a.relationship_ids, rels, "relationship", a.order),
            };
            (!step.element_ids.is_empty() || !step.relationship_ids.is_empty()).then_some(step)
        })
        .collect()
}

/// Graft element x/y + relationship routing geometry onto one view's content.
fn graft_content(
    new: &mut ViewContent,
    old: &ViewContent,
    view_key: &str,
    elems: &Maps,
    rels: &Maps,
    report: &mut Report,
) {
    // id-keyed lookups replace the per-entry `find`/`any`
    // scans. `entry.or_insert` keeps the FIRST entry per id — exactly
    // what the old `find` answered (duplicate view entries are a validation
    // violation, but reconcile runs BEFORE validation, so the first-wins
    // pick must stay).
    let mut old_elem_by_id = HashMap::new();
    for oe in &old.elements {
        old_elem_by_id.entry(&oe.id).or_insert(oe);
    }
    let mut old_rel_by_id = HashMap::new();
    for orv in &old.relationships {
        old_rel_by_id.entry(&orv.id).or_insert(orv);
    }
    // The new view's membership wins; geometry comes from the layout.
    for e in &mut new.elements {
        if let Some(old_id) = elems.to_old.get(&e.id)
            && let Some(oe) = old_elem_by_id.get(old_id)
        {
            e.x = oe.x;
            e.y = oe.y;
            report.elements += 1;
        }
    }
    for r in &mut new.relationships {
        if let Some(old_id) = rels.to_old.get(&r.id)
            && let Some(or) = old_rel_by_id.get(old_id)
        {
            // Geometry fields only: description/url/order/response are the
            // DSL's statement of the view, not layout.
            r.vertices.clone_from(&or.vertices);
            r.routing = or.routing;
            r.jump = or.jump;
            r.position = or.position;
            report.relationships += 1;
        }
    }
    // Dropped: layout entries the graft could not place (the id sets are
    // built only HERE — after the graft loops' mutable borings of `new`).
    let new_elem_ids: HashSet<&Id> = new.elements.iter().map(|e| &e.id).collect();
    let new_rel_ids: HashSet<&Id> = new.relationships.iter().map(|r| &r.id).collect();
    for oe in &old.elements {
        if !elems
            .to_new
            .get(&oe.id)
            .is_some_and(|nid| new_elem_ids.contains(nid))
        {
            report.dropped.push(format!(
                "view \"{view_key}\" element `{}` — no longer in the view",
                oe.id
            ));
        }
    }
    for orv in &old.relationships {
        if !rels
            .to_new
            .get(&orv.id)
            .is_some_and(|nid| new_rel_ids.contains(nid))
        {
            report.dropped.push(format!(
                "view \"{view_key}\" relationship `{}` — no longer in the view",
                orv.id
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidizr_dsl::engine::{ParseOptions, parse_dsl};

    fn parse(text: &str) -> Workspace {
        let r = parse_dsl(
            text,
            ParseOptions {
                file: None,
                base_dir: None,
                json_loader: None,
                env_lookup: None,
                installed_themes: None,
            },
        )
        .unwrap_or_else(|e| panic!("parse: {e}"));
        assert!(r.errors.is_empty(), "fixture diagnostics: {:?}", r.errors);
        r.workspace
    }

    /// Stamp distinctive geometry on every element entry of view `key`
    /// (landscape + context views are all the fixtures use).
    fn stamp(ws: &mut Workspace, key: &str) {
        let stamp_one = |content: Option<&mut ViewContent>| {
            if let Some(c) = content {
                for (i, e) in c.elements.iter_mut().enumerate() {
                    e.x = 100 + i32::try_from(i).unwrap();
                    e.y = 200 + i32::try_from(i).unwrap();
                }
            }
        };
        let vs = &mut ws.views;
        for v in &mut vs.system_landscape_views {
            if v.base.key == key {
                stamp_one(Some(&mut v.content));
            }
        }
        for v in &mut vs.system_context_views {
            if v.base.key == key {
                stamp_one(Some(&mut v.content));
            }
        }
    }

    fn landscape(ws: &Workspace) -> &oxidizr_core::views::SystemLandscapeView {
        ws.views
            .system_landscape_views
            .first()
            .expect("landscape view")
    }

    #[test]
    fn remapped_reports_the_full_uncontested_pairs() {
        // Same id-shift fixture: identifiers carry the match, so BOTH
        // elements remap (api 1→2, web 2→3) and the relationship
        // follows (3→4) — the viewer's overlay re-key consumes exactly
        // these lists (§).
        let layout = parse(
            "workspace \"W\" {\n    model {\n        api = softwareSystem \"API\"\n        web = softwareSystem \"Web\"\n        api -> web \"Uses\"\n    }\n}\n",
        );
        let updated = parse(
            "workspace \"W\" {\n    model {\n        db = softwareSystem \"DB\"\n        api = softwareSystem \"API\"\n        web = softwareSystem \"Web\"\n        api -> web \"Uses\"\n    }\n}\n",
        );
        let (_, report) = reconcile(&layout, &updated);
        let elements: Vec<(String, String)> = report
            .remapped_elements
            .iter()
            .map(|(old, new)| (old.to_string(), new.to_string()))
            .collect();
        assert!(
            elements.contains(&("1".to_string(), "2".to_string())),
            "api 1→2: {elements:?}"
        );
        assert!(
            elements.contains(&("2".to_string(), "3".to_string())),
            "web 2→3: {elements:?}"
        );
        assert!(
            !elements.iter().any(|(_, new)| new == "1"),
            "the inserted db has no layout predecessor: {elements:?}"
        );
        // Relationship ids OVERLAP element ids (both spaces mint "1",
        // "2", …) — which is exactly why the lists are per kind: the
        // rel pair (3→4) could not live in the element list.
        let relationships: Vec<(String, String)> = report
            .remapped_relationships
            .iter()
            .map(|(old, new)| (old.to_string(), new.to_string()))
            .collect();
        assert_eq!(relationships, vec![("3".to_string(), "4".to_string())]);
    }

    #[test]
    fn identifier_match_survives_id_shift() {
        // Old ids: api=1, web=2. Edited: a system inserted first, so api=2,
        // web=3 — identifiers (the `api =` assignments) carry the match.
        let mut layout = parse(
            "workspace \"W\" {\n    model {\n        api = softwareSystem \"API\"\n        web = softwareSystem \"Web\"\n    }\n    views {\n        systemLandscape \"L\" {\n            include api\n            include web\n        }\n    }\n}\n",
        );
        stamp(&mut layout, "L");
        let updated = parse(
            "workspace \"W\" {\n    model {\n        db = softwareSystem \"DB\"\n        api = softwareSystem \"API\"\n        web = softwareSystem \"Web\"\n    }\n    views {\n        systemLandscape \"L\" {\n            include api\n            include web\n        }\n    }\n}\n",
        );
        let (out, report) = reconcile(&layout, &updated);
        assert!(report.ambiguous.is_empty(), "{:?}", report.ambiguous);
        assert_eq!(report.dropped, Vec::<String>::new());
        assert_eq!(report.elements, 2);
        let l = landscape(&out);
        let by_id: HashMap<_, _> = l
            .content
            .elements
            .iter()
            .map(|e| (e.id.to_string(), (e.x, e.y)))
            .collect();
        // New ids: db=1, api=2, web=3 — api keeps the stamped (100, 200).
        assert_eq!(by_id["2"], (100, 200));
        assert_eq!(by_id["3"], (101, 201));
    }

    #[test]
    fn canonical_fallback_after_identifier_rename() {
        // Identifier renamed away (`api` → `backend`); the name is stable,
        // so the canonical-name fallback carries the match (ids shifted too).
        let mut layout = parse(
            "workspace \"W\" {\n    model {\n        api = softwareSystem \"API\"\n    }\n    views {\n        systemLandscape \"L\" {\n            include api\n        }\n    }\n}\n",
        );
        stamp(&mut layout, "L");
        let updated = parse(
            "workspace \"W\" {\n    model {\n        other = softwareSystem \"Other\"\n        backend = softwareSystem \"API\"\n    }\n    views {\n        systemLandscape \"L\" {\n            include backend\n        }\n    }\n}\n",
        );
        let (out, report) = reconcile(&layout, &updated);
        assert!(report.ambiguous.is_empty(), "{:?}", report.ambiguous);
        assert_eq!(report.elements, 1);
        let e = &landscape(&out).content.elements[0];
        assert_eq!((e.id.to_string(), e.x, e.y), ("2".to_string(), 100, 200));
    }

    #[test]
    fn contested_claim_is_ambiguous_never_guessed() {
        // Old: `x = "Foo"` + `y = "Bar"`; edited: one system that took the
        // identifier of the first AND the name of the second (`x = "Bar"`).
        // Old id 1 resolves to new id 1 by identifier, old id 2 resolves to
        // the SAME new id by canonical name — the contest revokes the first
        // claim and poisons the new id: NOTHING is guessed, geometry stays 0.
        let mut layout = parse(
            "workspace \"W\" {\n    model {\n        x = softwareSystem \"Foo\"\n        y = softwareSystem \"Bar\"\n    }\n    views {\n        systemLandscape \"L\" {\n            include x\n            include y\n        }\n    }\n}\n",
        );
        stamp(&mut layout, "L");
        let updated = parse(
            "workspace \"W\" {\n    model {\n        x = softwareSystem \"Bar\"\n    }\n    views {\n        systemLandscape \"L\" {\n            include x\n        }\n    }\n}\n",
        );
        let (out, report) = reconcile(&layout, &updated);
        assert_eq!(report.ambiguous.len(), 1);
        let msg = &report.ambiguous[0].message;
        assert!(msg.starts_with("ambiguous reconcile:"), "{msg}");
        assert!(msg.contains("new id `1`"), "{msg}");
        // Both claims revoked: the single new element keeps its parsed 0/0
        // and both layout entries report as dropped.
        assert_eq!(report.elements, 0);
        assert_eq!(
            report
                .dropped
                .iter()
                .filter(|d| d.contains("element `") && d.contains("no longer in the view"))
                .count(),
            2,
            "{:?}",
            report.dropped
        );
        assert_eq!(landscape(&out).content.elements[0].x, 0);
    }

    #[test]
    fn kind_prefixed_canonicals_do_not_collide() {
        // A person AND a software system named "Bob" have DIFFERENT canonical
        // names (`Person://Bob` vs `SoftwareSystem://Bob`), so dropping the
        // person is an unambiguous deletion — the system's geometry grafts.
        let mut layout = parse(
            "workspace \"W\" {\n    model {\n        pbob = person \"Bob\"\n        bob = softwareSystem \"Bob\"\n    }\n    views {\n        systemLandscape \"L\" {\n            include pbob\n            include bob\n        }\n    }\n}\n",
        );
        stamp(&mut layout, "L");
        let updated = parse(
            "workspace \"W\" {\n    model {\n        bob = softwareSystem \"Bob\"\n    }\n    views {\n        systemLandscape \"L\" {\n            include bob\n        }\n    }\n}\n",
        );
        let (out, report) = reconcile(&layout, &updated);
        assert!(report.ambiguous.is_empty(), "{:?}", report.ambiguous);
        assert_eq!(report.elements, 1);
        let e = &landscape(&out).content.elements[0];
        assert_eq!((e.id.to_string(), e.x, e.y), ("1".to_string(), 101, 201));
        // The dropped person's layout entry is reported, not an ambiguity.
        assert!(
            report
                .dropped
                .iter()
                .any(|d| d.contains("element `1`") && d.contains("no longer in the view")),
            "{:?}",
            report.dropped
        );
    }

    #[test]
    fn missing_views_and_elements_are_reported_as_dropped() {
        // Layout has an extra view and an element the new view excludes.
        let mut layout = parse(
            "workspace \"W\" {\n    model {\n        a = softwareSystem \"A\"\n        b = softwareSystem \"B\"\n    }\n    views {\n        systemLandscape \"L\" {\n            include a\n            include b\n        }\n        systemLandscape \"Gone\" {\n            include a\n        }\n    }\n}\n",
        );
        stamp(&mut layout, "L");
        let updated = parse(
            "workspace \"W\" {\n    model {\n        a = softwareSystem \"A\"\n        b = softwareSystem \"B\"\n    }\n    views {\n        systemLandscape \"L\" {\n            include a\n        }\n    }\n}\n",
        );
        let (_out, report) = reconcile(&layout, &updated);
        assert_eq!(report.elements, 1);
        assert!(
            report
                .dropped
                .iter()
                .any(|d| d == "view \"Gone\" — not in the re-parsed workspace"),
            "{:?}",
            report.dropped
        );
        assert!(
            report
                .dropped
                .iter()
                .any(|d| d.contains("view \"L\"") && d.contains("element `2`")),
            "{:?}",
            report.dropped
        );
    }

    #[test]
    fn relationships_carry_routing_geometry() {
        let mut layout = parse(
            "workspace \"W\" {\n    model {\n        a = softwareSystem \"A\"\n        b = softwareSystem \"B\"\n        a -> b \"Uses\"\n    }\n    views {\n        systemLandscape \"L\" {\n            include *\n        }\n    }\n}\n",
        );
        for v in &mut layout.views.system_landscape_views {
            if v.base.key == "L" {
                v.content.relationships[0]
                    .vertices
                    .push(oxidizr_core::views::Vertex { x: 42, y: 7 });
            }
        }
        // Inserted first: every old id shifts by one.
        let updated = parse(
            "workspace \"W\" {\n    model {\n                z = softwareSystem \"Z\"\n        a = softwareSystem \"A\"\n        b = softwareSystem \"B\"\n        a -> b \"Uses\"\n    }\n    views {\n        systemLandscape \"L\" {\n            include *\n        }\n    }\n}\n",
        );
        let (out, report) = reconcile(&layout, &updated);
        assert!(report.ambiguous.is_empty(), "{:?}", report.ambiguous);
        assert_eq!(report.relationships, 1);
        let rv = &out.views.system_landscape_views[0].content.relationships[0];
        assert_eq!(rv.id.to_string(), "4"); // shared counter: z=1, a=2, b=3, rel=4
        assert_eq!(rv.vertices[0].x, 42);
    }

    #[test]
    fn animations_are_remapped() {
        let mut layout = parse(
            "workspace \"W\" {\n    model {\n        a = softwareSystem \"A\"\n        b = softwareSystem \"B\"\n    }\n    views {\n        systemLandscape \"L\" {\n            include a\n            include b\n        }\n    }\n}\n",
        );
        for v in &mut layout.views.system_landscape_views {
            v.animations.push(Animation {
                order: 1,
                element_ids: vec!["1".into()],
                relationship_ids: vec![],
            });
        }
        let updated = parse(
            "workspace \"W\" {\n    model {\n        z = softwareSystem \"Z\"\n        a = softwareSystem \"A\"\n        b = softwareSystem \"B\"\n    }\n    views {\n        systemLandscape \"L\" {\n            include a\n            include b\n        }\n    }\n}\n",
        );
        let (out, report) = reconcile(&layout, &updated);
        assert!(report.ambiguous.is_empty(), "{:?}", report.ambiguous);
        assert_eq!(
            out.views.system_landscape_views[0].animations[0].element_ids,
            ["2"]
        );
    }

    #[test]
    fn empty_after_remap_step_is_dropped() {
        let mut layout = parse(
            "workspace \"W\" {\n    model {\n        a = softwareSystem \"A\"\n        gone = softwareSystem \"Gone\"\n    }\n    views {\n        systemLandscape \"L\" {\n            include a\n        }\n    }\n}\n",
        );
        for v in &mut layout.views.system_landscape_views {
            v.animations.push(Animation {
                order: 1,
                element_ids: vec!["2".into()],
                relationship_ids: vec![],
            });
        }
        let updated = parse(
            "workspace \"W\" {\n    model {\n        a = softwareSystem \"A\"\n    }\n    views {\n        systemLandscape \"L\" {\n            include a\n        }\n    }\n}\n",
        );
        let (out, report) = reconcile(&layout, &updated);
        assert_eq!(out.views.system_landscape_views[0].animations, Vec::new());
        assert!(
            report
                .dropped
                .iter()
                .any(|d| d.contains("animation step 1") && d.contains("gone")),
            "{:?}",
            report.dropped
        );
    }
}
