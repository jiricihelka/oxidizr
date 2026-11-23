//! "did you mean …?" decoration (the decoration freedom; home moved to
//! `oxidizr-api` because the command bodies that decorate with it
//! live in the headless pipeline).
//!
//! Computed at the API boundary — the engine's `Issue` message stays the
//! the official tooling text byte-for-byte; only [`Issue::help`] gains the suggestion.
//! Both candidate sets come from what is already on hand here:
//!
//! * **scope-permitted keywords** — the official tooling prints them verbatim in
//!   `Unexpected tokens (expected: a, b, c)`, so parsing the message recovers
//!   the set the engine checked (no engine-side plumbing needed);
//! * **known identifiers** — the `structurizr.dsl.identifier` properties of
//!   the parsed workspace (plus space-free element names, which the official tooling
//!   also accepts as references for anonymous elements).

use oxidizr_core::Workspace;
use oxidizr_core::builder::WorkspaceBuilder;
use oxidizr_core::validate::flattened;

use crate::Issue;

/// The DSL-stamped identity property (a locked compat decision).
const DSL_IDENTIFIER: &str = "structurizr.dsl.identifier";

/// Case-insensitive Levenshtein distance (two-row DP, chars).
fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.to_lowercase().chars().collect();
    let b: Vec<char> = b.to_lowercase().chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = prev[j] + usize::from(ca != cb);
            cur[j + 1] = cur[j + 1].min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// The accept bound grows with the token: 1 edit for short names, 2 for
/// ordinary ones, 3 for long descriptive identifiers.
fn max_distance(token: &str) -> usize {
    match token.chars().count() {
        0..=4 => 1,
        5..=8 => 2,
        _ => 3,
    }
}

/// The closest candidate within [`max_distance`] (deterministic: the first
/// best in candidate order).
fn closest<'a>(token: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let bound = max_distance(token);
    let mut best: Option<(usize, &str)> = None;
    for cand in candidates {
        let d = distance(token, cand);
        if d <= bound && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, cand));
        }
    }
    best.map(|(_, c)| c)
}

/// Every name a statement COULD have referenced: the DSL identifiers of all
/// model items and relationships, plus space-free element names (anonymous
/// elements are referenceable by name; their uuid identifiers are not).
fn known_names(ws: &Workspace) -> Vec<String> {
    let mut b = WorkspaceBuilder::new();
    b.adopt(ws.clone());
    let mut out: Vec<String> = flattened(&ws.model)
        .iter()
        .filter_map(|(id, _, name)| {
            b.item(id)
                .and_then(|i| i.properties.get(DSL_IDENTIFIER))
                .map(str::to_string)
                .or_else(|| (!name.contains(' ')).then(|| name.clone()))
        })
        .collect();
    out.extend(
        ws.model
            .relationships
            .iter()
            .filter_map(|r| r.properties.get(DSL_IDENTIFIER).map(str::to_string)),
    );
    out
}

/// First quoted span of the message, if any.
fn quoted(message: &str) -> Option<&str> {
    let rest = message.split('\"').nth(1)?;
    (!rest.is_empty()).then_some(rest)
}

/// The statement keyword(s) of the snippet's logical line — the tokens a
/// catch-all rejection actually rejected (`id =` prefix stripped, like the
/// engine's token sublist).
fn statement_tokens(issue: &Issue) -> Vec<String> {
    let Some(snip) = &issue.snippet else {
        return Vec::new();
    };
    let line = &snip.text;
    let mut tokens: Vec<String> = line.split_whitespace().map(str::to_string).collect();
    // Skip the `id =` prefix: the keyword is the
    // third token, the catch-all rejected the list starting AT the keyword.
    if tokens.len() >= 3 && tokens[1] == "=" {
        tokens.drain(0..2);
    }
    tokens
}

/// Decorate one issue with a `did you mean …?` help line . `ws` is
/// the parsed workspace when the command has one (identifier candidates);
/// the keyword family works without it.
pub fn decorate(issue: Issue, ws: Option<&Workspace>) -> Issue {
    if issue.help.is_some() {
        return issue;
    }
    // plain `&&` let-chain — the old `(bool, Option)` tuple
    // pattern smuggled this exact guard (message contains the prefix AND
    // the `(expected: ` list is extractable); both legs are pure string
    // ops, so the short-circuit changes nothing observable.
    let help = if issue.message.contains("Unexpected tokens (expected: ")
        && let Some(list) = issue
            .message
            .split_once(" (expected: ")
            .map(|(_, rest)| rest.split(')').next().unwrap_or(rest))
    {
        let candidates: Vec<&str> = list
            .split(", ")
            .map(|c| c.split(' ').next().unwrap_or(c))
            .collect();
        statement_tokens(&issue)
            .iter()
            .find_map(|tok| closest(tok, candidates.iter().copied()))
            .map(|c| format!("did you mean \"{c}\"?"))
    } else if (issue.message.contains("does not exist")
        || (issue.message.contains("identified by")
            && issue.message.contains("could not be found")))
        && let Some(token) = quoted(&issue.message)
        && let Some(c) = ws
            .map(known_names)
            .as_deref()
            .and_then(|names| closest(token, names.iter().map(String::as_str)))
    {
        Some(format!("did you mean \"{c}\"?"))
    } else {
        None
    };
    Issue { help, ..issue }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_is_edit_semantics() {
        assert_eq!(distance("person", "person"), 0);
        assert_eq!(distance("person", "preson"), 2);
        assert_eq!(distance("person", "Person"), 0);
        assert_eq!(distance("", "abc"), 3);
    }

    #[test]
    fn bound_scales_with_token() {
        assert_eq!(max_distance("per"), 1);
        assert_eq!(max_distance("person"), 2);
        assert_eq!(max_distance("deploymentEnvironment"), 3);
    }

    #[test]
    fn closest_picks_within_bound() {
        let c = ["person", "softwareSystem", "container"];
        assert_eq!(closest("persn", c), Some("person"));
        assert_eq!(closest("contaner", c), Some("container"));
        assert_eq!(closest("zzzzzzzz", c), None);
    }

    fn ws_api() -> Workspace {
        oxidizr_dsl::engine::parse_dsl(
            "workspace \"W\" {\n    model {\n        api = softwareSystem \"API\"\n    }\n}\n",
            oxidizr_dsl::engine::ParseOptions {
                file: None,
                base_dir: None,
                json_loader: None,
                env_lookup: None,
                installed_themes: None,
            },
        )
        .unwrap()
        .workspace
    }

    #[test]
    fn unknown_element_gets_identifier_suggestion() {
        let ws = ws_api();
        let issue = Issue::error(
            oxidizr_diag::codes::STATEMENT,
            "The element \"app\" does not exist",
        );
        let out = decorate(issue, Some(&ws));
        assert_eq!(out.help.as_deref(), Some("did you mean \"api\"?"));
    }

    #[test]
    fn unexpected_token_gets_keyword_suggestion() {
        let issue = Issue::error(
            oxidizr_diag::codes::UNEXPECTED_TOKEN,
            "Unexpected tokens (expected: archetypes, !identifiers, group, person, \
             softwareSystem, deploymentEnvironment, element, ->)",
        )
        .with_snippet("t.dsl", "softwareSyste \"X\"");
        let out = decorate(issue, None);
        assert_eq!(
            out.help.as_deref(),
            Some("did you mean \"softwareSystem\"?")
        );
    }

    #[test]
    fn assignment_prefix_stripped_for_keyword_lookup() {
        let issue = Issue::error(
            oxidizr_diag::codes::UNEXPECTED_TOKEN,
            "Unexpected tokens (expected: person, softwareSystem)",
        )
        .with_snippet("t.dsl", "s = softwareSyste \"X\"");
        let out = decorate(issue, None);
        assert_eq!(
            out.help.as_deref(),
            Some("did you mean \"softwareSystem\"?")
        );
    }

    #[test]
    fn far_misses_get_no_suggestion() {
        let issue = Issue::error(
            oxidizr_diag::codes::STATEMENT,
            "The element \"zzz\" does not exist",
        );
        let out = decorate(issue, Some(&ws_api()));
        assert_eq!(out.help, None);
    }
}
