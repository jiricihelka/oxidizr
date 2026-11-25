//! Chain section: the `configuration` scope (with its `scope`/`visibility`
//! statements and the `users` scope plus its ANY-token role catch-all), the
//! four `!docs` scopes and their `exclude` statement, the four
//! `!adrs`/`!decisions` scopes and their `exclude`, the removed `!constant`
//! keyword, `!const`/`!var` (substitution itself lives in the parse loop),
//! and `!identifiers <flat|hierarchical>`.
//!
//! `!docs`/`!adrs` capture the path token, the importer FQN (alias-mapped
//! for `!adrs`) and the `exclude` tokens into the pushed context; the import
//! runs when the documentation/decisions context ends
//! ([`Parser::end_context`] → `run_import`). Documentation and decisions
//! support is always enabled here, so the official tooling's "feature not
//! enabled" branches are unreachable.

use oxidizr_core::WorkspaceScope;

use super::ctx::{Ctx, Holder};
use super::error::DslError;
use super::{IdentifierScope, NameValue, NameValueType, Parser, Sec, Stmt, Tokens, eqi, tok};

pub(crate) fn section(p: &mut Parser, stmt: &Stmt) -> Sec {
    let first = stmt.first();
    let raw = &stmt.tokens;
    let t = raw.without_context_start_token();
    let open = Parser::should_start_context(raw);

    let is_workspace = p.in_ctx(|c| matches!(c, Ctx::Workspace));
    let is_model = p.in_ctx(|c| matches!(c, Ctx::Model { .. }));
    let is_config = p.in_ctx(|c| matches!(c, Ctx::Configuration));
    let is_users = p.in_ctx(|c| matches!(c, Ctx::Users));
    let is_docs = p.in_ctx(|c| matches!(c, Ctx::Documentation { .. }));
    let is_decisions = p.in_ctx(|c| matches!(c, Ctx::Decisions { .. }));

    // configuration { — the official tooling starts the context
    // UNCONDITIONALLY (no `should_start_context` gate).
    if eqi(&first, tok::CONFIGURATION) && is_workspace {
        p.start_context(Ctx::Configuration);
        return Sec::Handled;
    }

    // scope / visibility inside configuration { (RAW tokens).
    if eqi(&first, tok::SCOPE) && is_config {
        return parse_scope(p, raw).into();
    }
    if eqi(&first, tok::VISIBILITY) && is_config {
        return parse_visibility(p, raw).into();
    }

    // users { — also an unconditional context start.
    if eqi(&first, tok::USERS) && is_config {
        p.start_context(Ctx::Users);
        return Sec::Handled;
    }

    // The users-role catch-all — ANY token inside `users {`.
    if is_users {
        return user_role(p, raw).into();
    }

    // !docs <path> [importer] — workspace / softwareSystem /
    // container / component scopes; the single-line form ends the context
    // immediately, which is where the deferred import error surfaces.
    if eqi(&first, tok::DOCS)
        && let Some(holder) = docs_holder(p)
    {
        return docs_arm(p, &t, open, holder).into();
    }
    if eqi(&first, tok::DOCS_EXCLUDE) && is_docs {
        return parse_exclude(p, raw).into();
    }

    // !adrs / !decisions <path> [type|importer].
    if (eqi(&first, tok::ADRS) || eqi(&first, tok::DECISIONS)) && docs_holder(p).is_some() {
        return decisions_arm(p, &t, open, docs_holder_expect(p)).into();
    }
    if eqi(&first, tok::DECISIONS_EXCLUDE) && is_decisions {
        return parse_exclude(p, raw).into();
    }

    // !constant — removed keyword, context-independent throw.
    if eqi(&first, tok::CONSTANT) {
        return Sec::Err(DslError::stmt(
            "!constant was previously deprecated, and has now been removed - please use !const or !var instead",
        ));
    }

    // !const / !var <name> <value>, with the constant/variable
    // duplicate-name rules.
    if eqi(&first, tok::CONST) {
        return (|| {
            let (name, value) = name_value(raw)?;
            if p.constants.contains_key(&name) {
                return Err(DslError::stmt(format!(
                    "A constant/variable \"{name}\" already exists"
                )));
            }
            p.constants.insert(
                name.clone(),
                NameValue {
                    name,
                    value,
                    vtype: NameValueType::Constant,
                },
            );
            Ok(())
        })()
        .into();
    }
    if eqi(&first, tok::VAR) {
        return (|| {
            let (name, value) = name_value(raw)?;
            if p.constants
                .get(&name)
                .is_some_and(|nv| nv.vtype == NameValueType::Constant)
            {
                return Err(DslError::stmt(format!(
                    "A constant \"{name}\" already exists"
                )));
            }
            p.constants.insert(
                name.clone(),
                NameValue {
                    name,
                    value,
                    vtype: NameValueType::Variable,
                },
            );
            Ok(())
        })()
        .into();
    }

    // !identifiers <flat|hierarchical> — workspace or model scope.
    if eqi(&first, tok::IDENTIFIERS) && (is_workspace || is_model) {
        return identifier_scope(p, raw).into();
    }

    Sec::No
}

// ----------------------------------------------------------------------
// helpers
// ----------------------------------------------------------------------

/// The `scope` statement (RAW tokens; the error text embeds the LOWERCASED
/// value — it is lowercased before comparing).
fn parse_scope(p: &mut Parser, tokens: &Tokens) -> Result<(), DslError> {
    if tokens.has_more_than(1) {
        return Err(DslError::stmt(
            "Too many tokens, expected: scope <landscape|softwaresystem|none>",
        ));
    }
    if !tokens.includes(1) {
        return Err(DslError::stmt(
            "Expected: scope <landscape|softwaresystem|none>",
        ));
    }
    let scope = tokens.get(1).to_lowercase();
    if eqi(&scope, "landscape") {
        p.builder.set_scope(WorkspaceScope::Landscape);
    } else if eqi(&scope, "softwaresystem") {
        p.builder.set_scope(WorkspaceScope::SoftwareSystem);
    } else if eqi(&scope, "none") {
        p.builder.clear_scope();
    } else {
        return Err(DslError::stmt(format!(
            "The scope \"{scope}\" is not valid"
        )));
    }
    Ok(())
}

/// The `visibility` statement.
fn parse_visibility(p: &mut Parser, tokens: &Tokens) -> Result<(), DslError> {
    if tokens.has_more_than(1) {
        return Err(DslError::stmt(
            "Too many tokens, expected: visibility <private|public>",
        ));
    }
    if !tokens.includes(1) {
        return Err(DslError::stmt("Expected: visibility <private|public>"));
    }
    let visibility = tokens.get(1).to_lowercase();
    if eqi(&visibility, "private") {
        p.builder.set_visibility(oxidizr_core::Visibility::Private);
    } else if eqi(&visibility, "public") {
        p.builder.set_visibility(oxidizr_core::Visibility::Public);
    } else {
        return Err(DslError::stmt(format!(
            "The visibility \"{visibility}\" is not valid"
        )));
    }
    Ok(())
}

/// The user-role statement: `<username> <read|write>`.
fn user_role(p: &mut Parser, tokens: &Tokens) -> Result<(), DslError> {
    if tokens.has_more_than(1) {
        return Err(DslError::stmt(
            "Too many tokens, expected: <username> <read|write>",
        ));
    }
    if tokens.size() != 2 {
        return Err(DslError::stmt("Expected: <username> <read|write>"));
    }
    let username = tokens.get(0);
    let role = tokens.get(1);
    let role = if eqi(&role, "write") {
        oxidizr_core::Role::ReadWrite
    } else if eqi(&role, "read") {
        oxidizr_core::Role::ReadOnly
    } else {
        return Err(DslError::stmt("The role should be \"read\" or \"write\""));
    };
    // An empty username is rejected by the model, with its own message.
    if username.trim().is_empty() {
        return Err(DslError::stmt("A username must be specified."));
    }
    p.builder.add_user(&username, role);
    Ok(())
}

/// The documentation owner the `!docs`/`!adrs` arms accept:
/// workspace, software system, container or component context.
fn docs_holder(p: &Parser) -> Option<Holder> {
    match p.top() {
        Some(Ctx::Workspace) => Some(Holder::Workspace),
        Some(
            Ctx::SoftwareSystem { id, .. } | Ctx::Container { id, .. } | Ctx::Component { id },
        ) => Some(Holder::Item(id.clone())),
        _ => None,
    }
}

/// `docs_holder` against the guard-checked context.
fn docs_holder_expect(p: &Parser) -> Holder {
    docs_holder(p).expect("guard checked")
}

/// The four `!docs` arms: validate `!docs <path> [importer-fqn]`, push the
/// context, mark the DSL non-portable, and end the context right away for
/// the single-line form — which is exactly where the import runs.
fn docs_arm(p: &mut Parser, tokens: &Tokens, open: bool, holder: Holder) -> Result<(), DslError> {
    if tokens.has_more_than(2) {
        return Err(DslError::stmt(
            "Too many tokens, expected: !docs <path> <fqn>",
        ));
    }
    if !tokens.includes(1) {
        return Err(DslError::stmt("Expected: !docs <path> <fqn>"));
    }
    // path = token 1, importer FQN = token 2, else the default
    // documentation importer.
    let path = tokens.get(1).into_owned();
    let fqn = if tokens.includes(2) {
        tokens.get(2).into_owned()
    } else {
        oxidizr_import::DEFAULT_DOCUMENTATION_IMPORTER.to_owned()
    };
    p.start_context(Ctx::Documentation {
        holder,
        path,
        fqn,
        excludes: Vec::new(),
    });
    if let Some(frame) = p.frame_portable.last_mut() {
        *frame = false;
    }
    if open { Ok(()) } else { p.end_context() }
}

/// The four `!adrs`/`!decisions` arms — they share the
/// `!docs <path> [type|fqn]` grammar shape (the error text always says
/// `!decisions`, even for the `!adrs` token).
fn decisions_arm(
    p: &mut Parser,
    tokens: &Tokens,
    open: bool,
    holder: Holder,
) -> Result<(), DslError> {
    if tokens.has_more_than(2) {
        return Err(DslError::stmt(
            "Too many tokens, expected: !decisions <path> <type|fqn>",
        ));
    }
    if !tokens.includes(1) {
        return Err(DslError::stmt("Expected: !decisions <path> <type|fqn>"));
    }
    // Alias-mapped importer: case-SENSITIVE alias lookup, unknown names
    // pass through, default = the `adrtools` importer.
    let path = tokens.get(1).into_owned();
    let fqn = if tokens.includes(2) {
        oxidizr_import::decision_importer_fqn(&tokens.get(2))
    } else {
        oxidizr_import::ADRTOLS_DECISION_IMPORTER.to_owned()
    };
    p.start_context(Ctx::Decisions {
        holder,
        path,
        fqn,
        excludes: Vec::new(),
    });
    if let Some(frame) = p.frame_portable.last_mut() {
        *frame = false;
    }
    if open { Ok(()) } else { p.end_context() }
}

/// The `exclude` statement: EVERY token after the keyword joins the
/// context's exclude set. The empty-entry check (`"A regex must be
/// provided."`) is a context-end behavior, surfaced alongside the import.
fn parse_exclude(p: &mut Parser, tokens: &Tokens) -> Result<(), DslError> {
    if !tokens.includes(1) {
        return Err(DslError::stmt(
            "Expected: exclude <filename|regex> [filename|regex]",
        ));
    }
    if let Some(excludes) = match p.stack.last_mut() {
        Some(Ctx::Documentation { excludes, .. } | Ctx::Decisions { excludes, .. }) => {
            Some(excludes)
        }
        _ => None,
    } {
        for i in 1..tokens.size() {
            excludes.push(tokens.get(i).into_owned());
        }
    }
    Ok(())
}

/// The `!const`/`!var` grammar: the error text embeds the KEYWORD token as
/// written; the name must FULLY match `[a-zA-Z0-9-_.]+`.
fn name_value(tokens: &Tokens) -> Result<(String, String), DslError> {
    let keyword = tokens.get(0);
    if tokens.has_more_than(2) {
        return Err(DslError::stmt(format!(
            "Too many tokens, expected: {keyword} <name> <value>"
        )));
    }
    if !tokens.includes(2) {
        return Err(DslError::stmt(format!(
            "Expected: {keyword} <name> <value>"
        )));
    }
    let name = tokens.get(1);
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err(DslError::stmt(
            "Constant/variable names must only contain the following characters: a-zA-Z0-9-_.",
        ));
    }
    Ok((name.into_owned(), tokens.get(2).into_owned()))
}

/// The `!identifiers` statement — workspace or model scope (RAW tokens).
fn identifier_scope(p: &mut Parser, tokens: &Tokens) -> Result<(), DslError> {
    if tokens.has_more_than(1) {
        return Err(DslError::stmt(
            "Too many tokens, expected: !identifiers <flat|hierarchical>",
        ));
    }
    if !tokens.includes(1) {
        return Err(DslError::stmt("Expected: !identifiers <flat|hierarchical>"));
    }
    let name = tokens.get(1);
    let scope = if eqi(&name, "flat") {
        IdentifierScope::Flat
    } else if eqi(&name, "hierarchical") {
        IdentifierScope::Hierarchical
    } else {
        return Err(DslError::stmt("Expected: !identifiers <flat|hierarchical>"));
    };
    p.ids.set_scope(scope);
    Ok(())
}
