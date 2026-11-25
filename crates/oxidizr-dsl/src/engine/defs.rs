//! The `group` object table and the element-definition registration legs of
//! the DSL parse driver: the `uid` bookkeeping for `ElementGroup` objects,
//! the `define_element` funnel and the group-context opener. The parse
//! driver itself stays in the parent module; the section arms call these
//! through `pub(crate)`.

use oxidizr_core::CoreError;
use oxidizr_core::Id;

use super::rel::RelArch;
use super::{Ctx, DslEnvironment, DslError, DslGroup, GroupRecord, Parser, Tokens};

impl Parser<'_> {
    /// Open the member set for a fresh `ElementGroup` object (a `group`
    /// statement); returns its `uid`. `parent_uid` is the enclosing group
    /// (`ElementGroup.parentGroup`).
    // `cast_possible_truncation` justification: uids count model items;
    // the u64→usize narrowing can not overflow any real corpus.
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn group_open(&mut self, parent_uid: Option<u64>) -> u64 {
        let uid = self.next_uid();
        // `next_uid` also serves pseudo identity; the record table is
        // indexed uid-1, so uids consumed elsewhere leave empty records.
        while self.groups.len() < uid as usize {
            self.groups.push(GroupRecord::default());
        }
        self.groups[uid as usize - 1].parent_uid = parent_uid;
        uid
    }

    /// `ElementGroup.addElement`: record membership, propagating to the
    /// parent group object exactly like the official parser.
    #[allow(clippy::cast_possible_truncation)] // uid counts model items (see group_open)
    pub(crate) fn group_add_member(&mut self, uid: u64, member: &Id) {
        let mut cur = Some(uid);
        while let Some(u) = cur {
            let idx = u as usize - 1;
            if idx >= self.groups.len() {
                break;
            }
            if !self.groups[idx].members.contains(member) {
                self.groups[idx].members.push(member.clone());
            }
            cur = self.groups[idx].parent_uid;
        }
    }

    /// `ElementGroup.getElements` — the (propagated) member set.
    #[allow(clippy::cast_possible_truncation)] // uid counts model items (see group_open)
    pub(crate) fn group_members(&self, uid: u64) -> Vec<Id> {
        self.groups
            .get(uid as usize - 1)
            .map(|r| r.members.clone())
            .unwrap_or_default()
    }

    // ------------------------------------------------------------------
    // element-definition and relationship-statement post-processing
    // ------------------------------------------------------------------

    /// The shared post-creation sequence of the person, software-system,
    /// container and component parsers
    /// (the custom-element parser deviates — see `stmt_model`):
    /// description is set UNCONDITIONALLY (`null` → `""`), then
    /// the combined archetype + token-part tags are added, then the archetype
    /// perspectives, and finally the enclosing context's group
    /// (`setGroup` + `group.addElement`, which propagates to parent
    /// group objects).
    ///
    /// `tags` is the official parser's combined list (archetype tags first,
    /// then the comma-split statement token, each entry trimmed and
    /// first-position-deduped by [`oxidizr_core::WorkspaceBuilder::add_tag_parts`]).
    pub(crate) fn define_element(
        &mut self,
        id: &Id,
        description: &str,
        tags: &[String],
        arch: &RelArch,
    ) -> Result<(), DslError> {
        let _ = self.builder.set_description(id, description);
        self.builder.add_tag_parts(id, tags);
        for (name, value) in &arch.properties {
            let _ = self.builder.add_item_property(id, name, value);
        }
        for p in &arch.perspectives {
            // The archetype application goes through the plural
            // `addPerspectives`, which delegates to the 3-arg
            // `addPerspective` — `Perspective.setName` rejects names that
            // are null/blank FIRST, then the duplicate guard fires (a
            // blank-named archetype perspective therefore fails HERE, on
            // application, not when the archetype block defined it).
            if p.name.trim().is_empty() {
                return Err(DslError::core(&CoreError::PerspectiveNameRequired));
            }
            self.builder
                .add_perspective(id, p.clone())
                .map_err(|e| DslError::core(&e))?;
        }
        let group = self.top().and_then(Ctx::group_payload).cloned();
        if let Some(group) = group {
            let _ = self.builder.set_group(id, &group.name);
            self.group_add_member(group.uid, id);
        }
        Ok(())
    }

    /// The relationship-statement registration block shared by the four
    /// chain arms: exactly ONE created relationship is registered (and a
    /// `{` opens a relationship context); several create a
    /// a relationships context only (the top of the stack).
    /// `raw` is the RAW token list — the context-open check reads the
    /// original tokens including the trailing `{`.
    pub(crate) fn register_rel_result(
        &mut self,
        identifier: Option<&str>,
        rels: Vec<Id>,
        raw: &Tokens,
    ) -> Result<(), DslError> {
        let open = Self::should_start_context(raw);
        if rels.len() == 1 {
            let id = rels[0].clone();
            self.register_identifier_relationship(identifier, &id, "")?;
            if open {
                self.start_context(Ctx::Rel { id });
            }
        } else if open {
            self.start_context(Ctx::Relationships { items: rels });
        }
        Ok(())
    }

    /// The group statement's context switch: re-pushes the TOP
    /// context of the stack carrying a FRESH group object (the
    /// five groupable context kinds, each reconstructed with the new
    /// payload).
    // Exactly one of the six exclusive
    // arms runs and it MOVES the by-value group into the replacement frame —
    // callers hand over a group they no longer use,
    // so the per-arm `clone` was redundant and `needless_pass_by_value`
    // has nothing to ask for (its allow removed with it).
    pub(crate) fn start_group_context(&mut self, top: &Ctx, group: DslGroup) {
        let replacement = match top {
            Ctx::Model { .. } => Ctx::Model { group: Some(group) },
            Ctx::SoftwareSystem { id, .. } => Ctx::SoftwareSystem {
                id: id.clone(),
                group: Some(group),
            },
            Ctx::Container { id, .. } => Ctx::Container {
                id: id.clone(),
                group: Some(group),
            },
            Ctx::Environment { env, .. } => Ctx::Environment {
                env: env.clone(),
                group: Some(group),
            },
            // A `group` inside `source -/> destination {` matches the
            // ENVIRONMENT arm (subclass match) and re-pushes a PLAIN
            // environment frame — the
            // NoRelInEnv nature (and its linked relationship) is lost.
            Ctx::NoRelInEnv { environment, .. } => Ctx::Environment {
                env: DslEnvironment {
                    name: environment.clone(),
                },
                group: Some(group),
            },
            Ctx::DeploymentNode { id, .. } => Ctx::DeploymentNode {
                id: id.clone(),
                group: Some(group),
            },
            other => {
                // Unreachable: callers only pass groupable payload-bearing
                // tops; defensive replay of the official type ladder.
                let _ = other;
                return;
            }
        };
        self.start_context(replacement);
    }
}
