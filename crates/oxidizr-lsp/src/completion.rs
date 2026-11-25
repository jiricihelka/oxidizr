//! `textDocument/completion` .
//!
//! [`completions`] is the pure heart: buffer text + position in, item list
//! out. The scope comes from [`oxidizr_dsl::engine::cursor_scope`] — the
//! REAL engine replayed over the lines before the cursor (keyword-per-
//! context, identifiers, view keys, style tags) — so the menu can never
//! disagree with what the parser would accept. This module adds only the
//! wire presentation: the [`DIRECTIVES`] set (statement-level `!` tokens),
//! a handful of statement snippets, and the UTF-16-aware inverse mapper
//! (`position_to_offset`, the mirror of `diagnostics::position`).
//!
//! Kinds are cosmetic (the editor filters), the SETS are the contract:
//! keywords → `Keyword`, directives → `Keyword` (label starts with `!`),
//! identifiers → `Variable`, view keys → `Property`, tags → `Field`,
//! snippets → `Snippet` (tab stops).

use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionResponse, InsertTextFormat, Position,
};
use oxidizr_dsl::engine::{DIRECTIVES, cursor_scope};

/// Byte offset of an LSP position: walk `position.line` line terminators,
/// then `position.character` **UTF-16 code units** into the line. Clamped
/// both ways (a line past the EOF lands on `text.len()`; a column past
/// the line end lands before its `\n`) — editors race mid-edit, and the
/// server must answer them, not error.
#[must_use]
pub(crate) fn position_to_offset(text: &str, position: Position) -> usize {
    let mut offset = 0usize;
    for _ in 0..position.line {
        let Some(newline) = text[offset..].find('\n') else {
            return text.len(); // a line past the EOF clamps to the end
        };
        offset += newline + 1;
    }
    let mut column = 0u32;
    for ch in text[offset..].chars() {
        if ch == '\n' || column >= position.character {
            break;
        }
        // Non-BMP chars are two UTF-16 units — the mirror of
        // `diagnostics::position`'s `encode_utf16().count()`.
        column += if ch.len_utf16() == 2 { 2 } else { 1 };
        offset += ch.len_utf8();
    }
    offset
}

/// The token being typed at `offset`: the identifier run ending there
/// (letters/digits/`_`/`-`, plus a leading `!` so a half-typed directive
/// filters the directive set). Empty at a fresh statement position.
#[must_use]
pub(crate) fn prefix_at(text: &str, offset: usize) -> &str {
    let offset = offset.min(text.len());
    let mut start = offset;
    while start > 0 {
        let Some(prev) = text[..start].chars().next_back() else {
            break;
        };
        if prev.is_ascii_alphanumeric() || matches!(prev, '_' | '-' | '!') {
            start -= prev.len_utf8();
        } else {
            break;
        }
    }
    &text[start..offset]
}

/// The completion menu for `text` at `position`. A `!`-prefix filters to
/// the directives; otherwise the context's keyword list plus the
/// identifiers/view keys/tags registered BEFORE the cursor, filtered
/// case-insensitively by the prefix being typed. Statement snippets join
/// when their own keyword is permitted here.
#[must_use]
pub fn completions(text: &str, position: Position) -> Vec<CompletionItem> {
    let scope = cursor_scope(text, position_to_offset(text, position));
    let prefix = prefix_at(text, position_to_offset(text, position));
    let directive_leg = prefix.starts_with('!');
    let mut items: Vec<CompletionItem> = Vec::new();
    if directive_leg || prefix.is_empty() {
        items.extend(
            DIRECTIVES
                .iter()
                .copied()
                .filter(|d| matches_prefix(d, prefix))
                .map(|d| item(d, CompletionItemKind::KEYWORD, "directive")),
        );
        if directive_leg {
            return items;
        }
    }
    items.extend(
        scope
            .keywords
            .iter()
            .copied()
            .filter(|k| matches_prefix(k, prefix))
            .map(|k| item(k, CompletionItemKind::KEYWORD, "keyword")),
    );
    items.extend(
        scope
            .identifiers
            .iter()
            .map(String::as_str)
            .filter(|i| matches_prefix(i, prefix))
            .map(|i| item(i, CompletionItemKind::VARIABLE, "identifier")),
    );
    items.extend(
        scope
            .view_keys
            .iter()
            .map(String::as_str)
            .filter(|v| matches_prefix(v, prefix))
            .map(|v| item(v, CompletionItemKind::PROPERTY, "view key")),
    );
    items.extend(
        scope
            .tags
            .iter()
            .map(String::as_str)
            .filter(|t| matches_prefix(t, prefix))
            .map(|t| item(t, CompletionItemKind::FIELD, "tag")),
    );
    // `directive_leg` is false here (it returns early above), so the
    // snippets face the plain prefix/gate filter like every other leg.
    items.extend(
        SNIPPETS
            .iter()
            .filter(|s| matches_prefix(s.label, prefix) && scope.keywords.contains(&s.gate))
            .map(snippet_item),
    );
    items
}

/// One wire item from a label.
fn item(label: &str, kind: CompletionItemKind, detail: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_owned(),
        kind: Some(kind),
        detail: Some(detail.to_owned()),
        ..CompletionItem::default()
    }
}

/// Case-insensitive `label` starts with `prefix` (the DSL compares tokens
/// case-insensitively, `eqi`; the editor filters again client-side).
fn matches_prefix(label: &str, prefix: &str) -> bool {
    label
        .to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
}

/// A statement snippet: `label` is the trigger word, `gate` the keyword
/// whose permission marks the context the snippet fits, `body` the
/// tab-stopped text.
struct Snippet {
    label: &'static str,
    gate: &'static str,
    body: &'static str,
}

/// The statement starters (L-4: `sys`, `rel`, …) — gated on the same
/// keyword the snippet inserts, so they surface exactly where valid.
const SNIPPETS: &[Snippet] = &[
    Snippet {
        label: "sys",
        gate: "softwareSystem",
        body: "softwareSystem \"${1:name}\" {\n\t${0}\n}",
    },
    Snippet {
        label: "per",
        gate: "person",
        body: "person \"${1:name}\"",
    },
    Snippet {
        label: "rel",
        // The Model scope admits the arrow form (there is no standalone
        // `relationship` statement keyword — the dump of L-2 proved it).
        gate: "->",
        body: "${1:from} -> ${2:to} \"${3:description}\"",
    },
];

fn snippet_item(snippet: &Snippet) -> CompletionItem {
    CompletionItem {
        label: snippet.label.to_owned(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("statement".to_owned()),
        insert_text: Some(snippet.body.to_owned()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..CompletionItem::default()
    }
}

/// The response payload for one request (the wire form: an item array).
#[must_use]
pub fn response(text: &str, position: Position) -> CompletionResponse {
    CompletionResponse::Array(completions(text, position))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Labels of the menu at a line/character, sorted for order-free
    /// assertions.
    fn labels(text: &str, line: u32, character: u32) -> Vec<String> {
        let mut out: Vec<String> = completions(text, Position::new(line, character))
            .into_iter()
            .map(|item| item.label)
            .collect();
        out.sort();
        out
    }

    fn has(labels: &[String], label: &str) -> bool {
        labels.iter().any(|l| l == label)
    }

    #[test]
    fn a_fresh_buffer_wants_the_workspace_keyword() {
        let menu = labels("", 0, 0);
        assert!(has(&menu, "workspace"), "{menu:?}");
        assert!(!has(&menu, "model"));
    }

    #[test]
    fn the_context_drives_the_keyword_leg() {
        let text = "workspace {\n\n";
        let menu = labels(text, 1, 0);
        assert!(has(&menu, "model") && has(&menu, "views"), "{menu:?}");
        assert!(
            !has(&menu, "person"),
            "no element tokens inside workspace: {menu:?}"
        );
        let text = "workspace {\n  model {\n\n";
        let menu = labels(text, 2, 0);
        assert!(
            has(&menu, "person") && has(&menu, "softwareSystem"),
            "{menu:?}"
        );
        assert!(!has(&menu, "views"));
    }

    #[test]
    fn prefixes_filter() {
        let text = "workspace {\n  model {\n    soft\n";
        // Cursor just after `soft` (line 2, 4 indent + 4 letters).
        let menu = labels(text, 2, 8);
        assert!(has(&menu, "softwareSystem"), "{menu:?}");
        assert!(!has(&menu, "person"), "the prefix cut it: {menu:?}");
    }

    #[test]
    fn a_bang_leg_offers_only_directives() {
        let text = "workspace {\n  !dec\n";
        let menu = labels(text, 1, 4);
        assert!(has(&menu, "!decisions"), "{menu:?}");
        assert!(!has(&menu, "model"));
        assert!(!has(&menu, "!constant"));
    }

    #[test]
    fn contextual_inventory_joins_the_menu() {
        let text = "workspace {\n  model {\n    p = person \"P\"\n\n";
        let menu = labels(text, 3, 0);
        assert!(has(&menu, "p"), "the registered id: {menu:?}");
    }

    #[test]
    fn snippets_are_gated_on_their_keyword() {
        let model = "workspace {\n  model {\n\n";
        let menu = labels(model, 2, 0);
        assert!(has(&menu, "sys") && has(&menu, "rel"), "{menu:?}");
        let root = "workspace {\n\n";
        let menu = labels(root, 1, 0);
        assert!(
            !has(&menu, "sys"),
            "no statement snippets at root: {menu:?}"
        );
    }

    #[test]
    fn position_maps_back_through_utf16() {
        // The mirror of `diagnostics::position` — same text as ITS test.
        let text = "ab\nĚx💔y\nok\n";
        for offset in 0..=text.len() {
            let p = crate::diagnostics::position(text, offset);
            assert_eq!(
                position_to_offset(text, p),
                offset.min(
                    // a mid-char offset maps to the boundary below, so the
                    // round trip pins char-aligned inputs only
                    {
                        let mut at = offset;
                        while at > 0 && !text.is_char_boundary(at) {
                            at -= 1;
                        }
                        at
                    }
                ),
                "offset {offset} -> {p:?}"
            );
        }
        // Past EOF clamps; a long line clamps at its end.
        assert_eq!(position_to_offset(text, Position::new(99, 0)), text.len());
        assert_eq!(position_to_offset("ab", Position::new(0, 9)), 2);
        assert_eq!(position_to_offset("", Position::new(0, 5)), 0);
    }

    #[test]
    fn prefix_walker_cuts_at_delimiters() {
        assert_eq!(prefix_at("  soft", 6), "soft");
        assert_eq!(prefix_at("  soft", 4), "so"); // mid-word walks what precedes
        assert_eq!(prefix_at("  soft", 2), ""); // a fresh statement position
        assert_eq!(prefix_at("!inc", 4), "!inc");
        assert_eq!(prefix_at("a-b_c", 5), "a-b_c");
        assert_eq!(prefix_at("q\"so", 4), "so");
    }
}
