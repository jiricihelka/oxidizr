//! Cursor-scope replay for the language server (§P4-LSP L-4).
//!
//! [`cursor_scope`] re-runs the REAL engine (tokenizer, `${}`-substitution,
//! the ordered dispatch chain, D1 recovery) over the lines BEFORE a cursor
//! offset and reports what the scope machine would accept there — never the
//! partial statement the cursor sits in (its prefix is the CLIENT's
//! filter material; feeding an incomplete statement would change the stack
//! in ways the finished file never would).
//!
//! The replay is filesystem-free BY CONSTRUCTION: [`ParseOptions::fragment`]
//! means no `dslFile`, and the `!include`/`extends` legs skip their whole
//! branch without one (the reference's no-file quirk, mirrored in
//! `stmt_line::include`). The env lookup is CLOSED (`None` for every name)
//! so `${VAR}` sites fail the same way on every machine — completion must
//! not depend on the editor server's environment.
//!
//! The returned [`CursorScope`] is purpose-shaped on purpose: `Ctx` and
//! `Parser` stay `pub(crate)` (`PHASE_1.md` 2R R-dsl-09 — externals get the
//! parse surface only), so the engine's working types never leak.

use oxidizr_core::Workspace;
use oxidizr_core::model::ModelItem;

use super::tok;
use super::{EnvLookup, ParseOptions, Parser};
use crate::preprocess;

/// What the scope machine accepts and knows AT a cursor offset (§P4-LSP
/// L-4): the completion menu's raw material.
#[derive(Debug, Clone)]
pub struct CursorScope {
    /// Tokens a statement may START with here — the top context's
    /// [`super::Ctx::permitted_tokens`] verbatim (empty where the engine
    /// lists nothing: a closed root, a `Comment` scope, …). With no open
    /// context and no `workspace` keyword yet, exactly `[workspace]` —
    /// the catch-all's own expectation.
    pub keywords: Vec<&'static str>,
    /// `true` when no context is open and no `workspace` keyword was seen
    /// (a file whose first token is still to come).
    pub expects_workspace: bool,
    /// Element + relationship identifiers the `id =` statements BEFORE the
    /// cursor registered (auto-UUID keys dropped — not typeable).
    pub identifiers: Vec<String>,
    /// View keys defined before the cursor, in the reference `getViews`
    /// order (style statements and `!ref` targets).
    pub view_keys: Vec<String>,
    /// Custom tags used before the cursor (elements + relationships),
    /// sorted and deduplicated — the `views { styles { … } }` vocabulary.
    pub tags: Vec<String>,
}

/// Every `!`-directive the parser dispatches. `!plugin`/`!script` are
/// deliberately absent: oxidizr rejects them as JVM-only, so advertising
/// them would offer unparseable statements. Their validity AT a given
/// scope is decided by the engine's dispatch chain — items are advisory.
pub const DIRECTIVES: &[&str] = &[
    tok::INCLUDE_FILE,
    tok::DOCS,
    tok::ADRS,
    tok::DECISIONS,
    tok::CONSTANT,
    tok::CONST,
    tok::VAR,
    tok::IDENTIFIERS,
    tok::IMPLIED_RELATIONSHIPS,
    tok::REF,
    tok::EXTEND,
    tok::FIND_ELEMENT,
    tok::FIND_ELEMENTS,
    tok::FIND_RELATIONSHIP,
    tok::FIND_RELATIONSHIPS,
    tok::COMPONENT_FINDER,
];

/// Replay `source` up to `cursor` (a BYTE offset into it; clamped into
/// range and down to a char boundary) and report the scope there. Cheap
/// enough for every keystroke on DSL-file sizes (one engine pass over the
/// lines before the cursor).
#[must_use]
pub fn cursor_scope(source: &str, cursor: usize) -> CursorScope {
    let mut cursor = cursor.min(source.len());
    while cursor > 0 && !source.is_char_boundary(cursor) {
        cursor -= 1;
    }
    let opts = ParseOptions {
        env_lookup: Some(EnvLookup::new(|_name| None)),
        ..ParseOptions::fragment()
    };
    let mut parser = Parser::new(opts);
    for logical in &preprocess::preprocess(source) {
        if logical.dropped {
            continue; // the reference's never-flushed EOF tail
        }
        if logical.span.1 > cursor {
            // `span` is (start, end); this line ends after the cursor, so
            // it contains or follows it. Its (partial) statement is never
            // fed — replay stops at the statement boundary the cursor has
            // not crossed yet.
            break;
        }
        // The `parse_lines` loop verbatim, recovery included: an errored
        // statement pops back to its line-entry stack so one broken line
        // does not blind the completion below it.
        let height = parser.stack.len();
        let outcome = (|| {
            let raw = parser.logical_raw(logical, source)?;
            parser.process_line(logical.number, raw)
        })();
        if let Err(e) = outcome {
            parser.stack.truncate(height);
            if e.fatal_file {
                break;
            }
        }
    }
    let expects_workspace = parser.stack.is_empty() && !parser.have_workspace;
    let keywords = match parser.stack.last() {
        Some(ctx) => ctx.permitted_tokens(),
        None if expects_workspace => vec![tok::WORKSPACE],
        None => Vec::new(),
    };
    let ws = parser.builder.workspace();
    CursorScope {
        keywords,
        expects_workspace,
        identifiers: parser.ids.all_identifiers().map(str::to_owned).collect(),
        view_keys: ws
            .views
            .all_views()
            .iter()
            .map(|v| v.key().to_owned())
            .collect(),
        tags: collect_tags(ws),
    }
}

/// Every stored (custom) tag of the model — the same `ModelItem` walk as
/// [`super::json_dsl_identifiers`], plus the relationship tags.
fn collect_tags(ws: &Workspace) -> Vec<String> {
    let m = &ws.model;
    let mut items: Vec<&ModelItem> = Vec::new();
    items.extend(m.custom_elements.iter().map(|e| &e.item));
    items.extend(m.people.iter().map(|e| &e.item));
    for s in &m.software_systems {
        items.push(&s.item);
        for c in &s.containers {
            items.push(&c.item);
            items.extend(c.components.iter().map(|k| &k.item));
        }
    }
    for n in &m.deployment_nodes {
        super::collect_json_item_refs(n, &mut items);
    }
    let mut tags: Vec<String> = items
        .iter()
        .flat_map(|item| item.tags.stored().iter().cloned())
        .chain(
            m.relationships
                .iter()
                .flat_map(|r| r.tags.stored().iter().cloned()),
        )
        .collect();
    tags.sort();
    tags.dedup();
    tags
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    /// Byte offset of the `offset`th occurrence of `needle`.
    fn nth(source: &str, needle: &str, offset: usize) -> usize {
        let mut at = 0;
        for _ in 0..=offset {
            at = source[at..].find(needle).unwrap() + at + 1;
        }
        at - 1
    }

    /// Pin the span shape the replay guard relies on: `(start, end)`,
    /// tiling the source (a wrong assumption here silently truncates the
    /// replay after the first line).
    #[test]
    fn logical_spans_are_start_end() {
        let source = "workspace {\n  model {\n";
        let logicals = preprocess::preprocess(source);
        assert_eq!(logicals[0].span, (0, 12));
        assert_eq!(logicals[1].span, (12, source.len()));
    }

    #[test]
    fn empty_source_expects_the_workspace_keyword() {
        let scope = cursor_scope("", 0);
        assert!(scope.expects_workspace);
        assert_eq!(scope.keywords, vec![tok::WORKSPACE]);
        assert_eq!(scope.identifiers, Vec::<String>::new());
    }

    #[test]
    fn root_after_the_close_offers_nothing() {
        // The catch-all has no list once `workspace` exists and the stack
        // is empty — completion adds nothing to a finished file.
        let source = "workspace {\n}\n";
        let scope = cursor_scope(source, source.len());
        assert!(!scope.expects_workspace);
        assert_eq!(scope.keywords, Vec::<&str>::new());
    }

    #[test]
    fn the_stack_at_the_cursor_drives_the_keywords() {
        let source = "workspace {\n  model {\n  }\n}\n";
        // Cursors sit on the NEXT line's start — the statement boundary
        // the replay stops at (the cursor's own line is never fed).
        // After `workspace {`: the Workspace list (model/views/…), no
        // element tokens yet.
        let scope = cursor_scope(source, nth(source, "\n", 0) + 1);
        assert!(scope.keywords.contains(&tok::MODEL));
        assert!(scope.keywords.contains(&tok::VIEWS));
        assert!(!scope.keywords.contains(&tok::PERSON));
        // Inside `model {`: the Model list.
        let scope = cursor_scope(source, nth(source, "\n", 1) + 1);
        assert!(scope.keywords.contains(&tok::PERSON));
        assert!(scope.keywords.contains(&tok::SOFTWARE_SYSTEM));
        assert!(!scope.keywords.contains(&tok::VIEWS));
        // After the model CLOSES: back to the Workspace list.
        let scope = cursor_scope(source, nth(source, "\n", 2) + 1);
        assert!(scope.keywords.contains(&tok::VIEWS));
        assert!(!scope.keywords.contains(&tok::PERSON));
    }

    #[test]
    fn lines_after_the_cursor_stay_unparsed() {
        // The identifiers/views of the TAIL must not leak above the cursor.
        let source = "workspace {\n  model {\n    p = person \"P\"\n  }\n  views {\n    systemLandscape \"key1\" \"L\" {\n    }\n  }\n}\n";
        let before = cursor_scope(source, nth(source, "p = person", 0));
        assert_eq!(before.identifiers, Vec::<String>::new());
        assert_eq!(before.view_keys, Vec::<String>::new());
        assert!(before.keywords.contains(&tok::PERSON));
        let after = cursor_scope(source, source.len());
        assert!(after.keywords.is_empty(), "the closed root lists nothing");
        assert!(after.identifiers.iter().any(|i| i == "p"));
        assert_eq!(after.view_keys, vec!["key1".to_owned()]);
    }

    #[test]
    fn the_cursor_line_is_never_fed() {
        // Cursor mid-`  model {`: the partial statement must not push the
        // Model context — completion still offers the Workspace list.
        let source = "workspace {\n  model {\n";
        let scope = cursor_scope(source, nth(source, "model", 0) + 3);
        assert!(scope.keywords.contains(&tok::MODEL));
        assert!(!scope.keywords.contains(&tok::PERSON));
    }

    #[test]
    fn broken_lines_do_not_blind_the_replay() {
        // D1 recovery mirrors the parse loop: the error statement pops
        // back to its line-entry stack, the lines below keep their scope.
        let source = "workspace {\n  this is not dsl ~~~~\n  model {\n";
        let scope = cursor_scope(source, source.len());
        assert!(
            scope.keywords.contains(&tok::PERSON),
            "{:?}",
            scope.keywords
        );
    }

    #[test]
    fn replay_is_filesystem_and_environment_free() {
        // fragment options: `!include` skips its whole branch (no file),
        // `${}` sites resolve to nothing, yet the SCOPE survives.
        let source = "workspace {\n  !include nowhere-on-earth.dsl\n  model {\n  }\n}\n";
        let scope = cursor_scope(source, source.len());
        assert!(scope.keywords.is_empty(), "{:?}", scope.keywords);
        assert!(!scope.expects_workspace);
        // A `${}` of an unset variable fails its statement, not the replay.
        let source = "workspace {\n  name \"${OXZ_LSP_UNSET_VAR}\"\n  model {\n  }\n}\n";
        let scope = cursor_scope(source, source.len());
        assert!(!scope.expects_workspace);
    }

    #[test]
    fn identifiers_and_tags_are_registered_state() {
        let source = "workspace {\n  model {\n    p = person \"P\" {\n      tags \"Team\"\n    }\n    s = softwareSystem \"S\"\n    p -> s \"Uses\" {\n      tags \"Async\"\n    }\n  }\n}\n";
        let scope = cursor_scope(source, source.len());
        assert!(scope.identifiers.iter().any(|i| i == "p"));
        assert!(scope.identifiers.iter().any(|i| i == "s"));
        assert_eq!(scope.tags, vec!["Async".to_owned(), "Team".to_owned()]);
    }

    #[test]
    fn cursors_clamp_never_panic() {
        let source = "workspace {\n  name \"Pří💔šerně\"\n}\n";
        for cursor in [0usize, 3, 20, 40, source.len(), source.len() + 100] {
            let _ = cursor_scope(source, cursor); // inside chars, past EOF…
        }
        // Mid-char offsets land on the boundary below, harmlessly.
        let s1 = cursor_scope(source, 21);
        let s2 = cursor_scope(source, 22);
        assert_eq!(s1.keywords, s2.keywords);
    }

    #[test]
    fn directives_cover_the_dispatched_bang_set() {
        assert!(DIRECTIVES.contains(&tok::INCLUDE_FILE));
        assert!(DIRECTIVES.contains(&tok::REF));
        assert!(
            !DIRECTIVES.contains(&tok::PLUGIN),
            "JVM-only rejections stay hidden"
        );
        assert!(!DIRECTIVES.contains(&tok::SCRIPT));
    }
}
