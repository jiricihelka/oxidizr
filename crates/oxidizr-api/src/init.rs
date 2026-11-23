//! `oxidizr init` scaffold templates.
//!
//! Everything the command writes is embedded at build time
//! (`include_str!`) — zero network, zero new deps. The templates use
//! ONLY grammar our parser pins (the workspace DSL arm tables): the
//! starter `workspace.dsl` files pass `validate` AND the canonical
//! `fmt` printer, and a `dsl2json` golden of each one rides the test
//! suite — template DRIFT breaks CI, which is the intended teeth.
//!
//! The command body is [`crate::pipeline`] (the command-body invariant); this
//! module is the pure template table only.

/// Starter content (`--template`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitTemplate {
    /// A single software system + landscape view.
    Minimal,
    /// The canonical 3-software-system C4 landscape (person, internal
    /// system with containers, two supporting systems, the three
    /// starter views + styles).
    C4,
}

/// `--template` token (the CLI value shares it).
#[must_use]
pub const fn kind_name(template: InitTemplate) -> &'static str {
    match template {
        InitTemplate::Minimal => "minimal",
        InitTemplate::C4 => "c4",
    }
}

/// One scaffold file: repo-relative path (POSIX separators — the report
/// is deterministic) + embedded content.
pub(crate) type ScaffoldFile = (&'static str, &'static str);

/// The files one template writes, in REPORT order. `workspace.dsl` is
/// per-template; the rest are shared (`templates/shared/`).
pub(crate) fn files(template: InitTemplate) -> [ScaffoldFile; 5] {
    let workspace = match template {
        InitTemplate::Minimal => include_str!("../templates/minimal/workspace.dsl"),
        InitTemplate::C4 => include_str!("../templates/c4/workspace.dsl"),
    };
    [
        ("workspace.dsl", workspace),
        ("README.md", include_str!("../templates/shared/README.md")),
        (".gitignore", include_str!("../templates/shared/gitignore")),
        (
            ".editorconfig",
            include_str!("../templates/shared/editorconfig"),
        ),
        (
            ".vscode/settings.json",
            include_str!("../templates/shared/vscode-settings.json"),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidizr_dsl::engine::ParseOptions;

    /// The template teeth: every starter `workspace.dsl` parses
    /// with ZERO recorded errors — grammar drift fails here, in the
    /// unit tier, before the CLI goldens and the `dsl2json` snapshot.
    #[test]
    fn workspace_templates_parse_clean() {
        for template in [InitTemplate::Minimal, InitTemplate::C4] {
            let files = files(template);
            let source = files[0].1;
            assert_eq!(files[0].0, "workspace.dsl");
            let parsed = oxidizr_dsl::engine::parse_dsl(source, ParseOptions::fragment())
                .unwrap_or_else(|e| panic!("{}: {e}", kind_name(template)));
            assert!(
                parsed.errors.is_empty(),
                "{}: {:?}",
                kind_name(template),
                parsed.errors
            );
        }
    }

    #[test]
    fn file_table_is_complete_and_unique() {
        for template in [InitTemplate::Minimal, InitTemplate::C4] {
            let files = files(template);
            let mut paths: Vec<&str> = files.iter().map(|f| f.0).collect();
            let all = paths.len();
            paths.sort_unstable();
            paths.dedup();
            assert_eq!(paths.len(), all, "duplicate scaffold path");
            assert!(
                files.iter().all(|(_, content)| !content.is_empty()),
                "empty scaffold file"
            );
        }
    }
}
