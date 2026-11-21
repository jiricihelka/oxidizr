//! Diagnostics on open/change (§P4-LSP L-3).
//!
//! [`collect`] is the pure heart: buffer text in, `lsp_types::Diagnostic`
//! list out. The pipeline is the exact CLI one — `oxidizr_api::open` over
//! [`Input::Bytes`] (in-memory, NO filesystem access) — so the squiggles
//! match `oxidizr validate` bullet-for-bullet. JSON buffers take the
//! `json2json` route (`Input::Bytes` has no extension, so `api::open`
//! would always parse DSL — the URI decides HERE).
//!
//! Span mapping ([`range`]): `Issue::snippet` carries `(offset, length)`
//! byte coords against ITS text (the CLI renders it through miette's
//! `SourceSpan`), which is the full buffer for JSON errors but only the
//! offending SOURCE LINE for engine errors. The mapper locates the snippet
//! text inside the buffer (`find`), then converts byte offsets to LSP
//! positions — 0-based lines, columns in **UTF-16 code units** (the LSP
//! definition; `oxidizr_diag::Position::col` counts chars and is 1-based,
//! so it is NOT reusable here).

use lsp_types::{
    CodeDescription, Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range, Uri,
};
use oxidizr_api::{ApiOptions, Input, Issue, LogItem, Snippet};
use oxidizr_core::Workspace;
use oxidizr_diag::Severity;

/// `maxNumberOfProblems` default (P4-D5; the initializationOption lands
/// with L-5). A pathological DSL sprays one error per line; an editor
/// gutter copes with 100.
pub const MAX_DIAGNOSTICS: usize = 100;

/// The registry document every `OXZ-nnnn` code links to (the `href` the
/// editor shows under the diagnostic code).
const DOCS_BASE: &str = "https://github.com/jiricihelka/oxidizr/blob/main/docs/diagnostics.md";

/// Collect the diagnostics for one buffer. `uri` decides the format:
/// `*.json` validates through `oxidizr_json::from_slice` (the `json2json`
/// read leg, hydrate validation included); everything else parses as DSL.
#[must_use]
pub fn collect(uri: &Uri, text: &str) -> Vec<Diagnostic> {
    let issues = if is_json(uri) {
        json_issues(text)
    } else {
        dsl_issues(text)
    };
    let mut out: Vec<Diagnostic> = issues
        .iter()
        .map(|issue| to_diagnostic(text, issue))
        .collect();
    out.truncate(MAX_DIAGNOSTICS);
    out
}

/// DSL route: the viewer-facing open — a partial parse reports every
/// statement error (D1 recovery), a fatal parse reports the failure.
fn dsl_issues(text: &str) -> Vec<Issue> {
    let opts = ApiOptions::default();
    let log = match oxidizr_api::open(&Input::Bytes(text.as_bytes().to_vec()), &opts) {
        Ok(opened) => opened.log,
        Err(outcome) => outcome.log,
    };
    log.into_iter()
        .filter_map(|item| match item {
            LogItem::Issue(issue) => Some(issue),
            LogItem::OkLine(_) | LogItem::NoteLine(_) => None,
        })
        .collect()
}

/// JSON route: `from_slice` errors carry serde's line/column as a byte
/// span over the FULL text; an Ok parse may still fail the structural
/// read or hydrate validation (all inside the same call).
fn json_issues(text: &str) -> Vec<Issue> {
    match oxidizr_json::from_slice(text.as_bytes()) {
        Ok(_) => Vec::new(),
        Err(error) => vec![Issue::from_json(&error, text, "")],
    }
}

/// Format route: `*.json` validates/formats as JSON, everything else is
/// DSL (shared by the diagnostics, hover and formatting pipelines).
pub(crate) fn is_json(uri: &Uri) -> bool {
    uri.as_str().to_ascii_lowercase().ends_with(".json")
}

/// The hover store's source (§P4-LSP L3): the buffer parsed to a
/// [`Workspace`] ONLY when the pipeline is CLEAN — DSL: `api::open`
/// succeeds AND the partial-open log carries NO issue (a D1 recovery
/// logged errors, the buffer is not trustworthy); JSON: `from_slice`
/// succeeds. Any failure is `None` — and the server then keeps the
/// PREVIOUS workspace for that URI, which is what makes hover survive
/// mid-typing.
#[must_use]
pub fn workspace(uri: &Uri, text: &str) -> Option<Workspace> {
    if is_json(uri) {
        return oxidizr_json::from_slice(text.as_bytes()).ok();
    }
    let opened = oxidizr_api::open(
        &Input::Bytes(text.as_bytes().to_vec()),
        &ApiOptions::default(),
    )
    .ok()?;
    opened
        .log
        .into_iter()
        .all(|item| !matches!(item, LogItem::Issue(_)))
        .then_some(opened.workspace)
}

/// One [`Issue`] → one LSP diagnostic: message and code ride VERBATIM
/// (reference/registry texts beat editor cosmetics), the range comes from
/// the snippet, the severity maps 1:1, `source` names us.
fn to_diagnostic(text: &str, issue: &Issue) -> Diagnostic {
    let code = issue.code.to_string();
    Diagnostic {
        range: issue.snippet.as_deref().map_or_else(
            || Range::new(Position::new(0, 0), Position::new(0, 0)),
            |snippet| range(text, snippet),
        ),
        severity: Some(match issue.severity {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
            Severity::Info => DiagnosticSeverity::INFORMATION,
        }),
        code: Some(NumberOrString::String(code.clone())),
        // A per-code anchor into the registry. `Uri` has no Default, so
        // a (constant-broken, cannot happen) parse failure simply drops
        // the description rather than the diagnostic.
        code_description: format!("{DOCS_BASE}#{code}")
            .parse::<Uri>()
            .ok()
            .map(|href| CodeDescription { href }),
        source: Some("oxidizr".to_owned()),
        message: issue.message.clone(),
        ..Diagnostic::default()
    }
}

/// Byte span (against `snippet.text` — `(offset, length)`, the miette
/// convention) → LSP range against `text`. The snippet text is located in
/// the buffer first (engine snippets are ONE source line, JSON snippets
/// the whole document); text that cannot be found (an error from a
/// `!include`d file) yields the neutral zero range at 0:0.
#[must_use]
pub fn range(text: &str, snippet: &Snippet) -> Range {
    let base = if snippet.text.is_empty() {
        0
    } else {
        match text.find(&snippet.text) {
            Some(at) => at,
            None => return Range::new(Position::new(0, 0), Position::new(0, 0)),
        }
    };
    let (offset, length) = snippet.span;
    let start = base.saturating_add(offset).min(text.len());
    let end = base
        .saturating_add(offset)
        .saturating_add(length)
        .min(text.len());
    Range::new(position(text, start), position(text, end))
}

/// Byte offset → LSP position: 0-based line, column in UTF-16 code units.
/// `byte` clamps to `text.len()` and DOWN to a char boundary (a
/// hand-computed JSON column could split a multi-byte character; slicing
/// must not panic).
#[must_use]
pub fn position(text: &str, byte: usize) -> Position {
    let mut byte = byte.min(text.len());
    while byte > 0 && !text.is_char_boundary(byte) {
        byte -= 1;
    }
    let prefix = &text[..byte];
    let line = prefix.matches('\n').count();
    let line_start = prefix.rfind('\n').map_or(0, |at| at + 1);
    let character = text[line_start..byte].encode_utf16().count();
    #[allow(clippy::cast_possible_truncation)] // no DSL file has 4 Gi lines
    {
        Position::new(line as u32, character as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidizr_diag::Code;

    fn uri(path: &str) -> Uri {
        format!("file:///ws/{path}").parse().unwrap()
    }

    #[test]
    fn valid_dsl_reports_nothing() {
        let text = "workspace {\n  model {\n  }\n}\n";
        let diags = collect(&uri("ws.dsl"), text);
        assert_eq!(diags, Vec::<Diagnostic>::new());
    }

    #[test]
    fn broken_dsl_flags_the_offending_line() {
        // Line 3 is the source line the engine quotes; the range must land
        // on it, not at the buffer start.
        let text = "workspace {\n  model {\n    this is not dsl ~~~~\n  }\n}\n";
        let diags = collect(&uri("ws.dsl"), text);
        assert!(!diags.is_empty(), "the broken statement must report");
        let first = &diags[0];
        assert_eq!(first.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(first.source.as_deref(), Some("oxidizr"));
        let NumberOrString::String(code) = first.code.clone().unwrap() else {
            panic!("the code is the OXZ string");
        };
        assert!(code.starts_with("OXZ-"), "{code}");
        assert_eq!(
            first.range.start.line, 2,
            "0-based line of the offending source line: {:?}",
            first.range
        );
        let description = first.code_description.as_ref().unwrap();
        assert!(description.href.as_str().starts_with(DOCS_BASE));
        assert!(
            description.href.as_str().ends_with(&code),
            "{}",
            description.href.as_str()
        );
    }

    #[test]
    fn json_buffers_validate_through_the_read_leg() {
        // .json URI ⇒ JSON, although Input::Bytes alone would say DSL.
        let broken = r#"{ "name": "ws", }"#;
        let diags = collect(&uri("ws.json"), broken);
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert!(
            diags[0].message.starts_with("failed to parse JSON:"),
            "{:?}",
            diags[0].message
        );
        assert_eq!(diags[0].range.start.line, 0);
        // The lenient read COERCES scalars (D3), so the next rejection
        // the leg adds is hydrate validation — duplicate element names.
        let duplicate = r#"{"name": "w", "model": {"people": [{"id": "1", "name": "a"}, {"id": "2", "name": "a"}], "relationships": []}}"#;
        let diags = collect(&uri("ws.json"), duplicate);
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert!(
            diags[0].message.starts_with("workspace validation failed:"),
            "{:?}",
            diags[0].message
        );
        // A sound workspace stays quiet.
        let good = r#"{ "name": "ws", "description": "", "version": "" }"#;
        let diags = collect(&uri("ws.json"), good);
        assert_eq!(diags, Vec::<Diagnostic>::new());
    }

    #[test]
    fn positions_count_utf16_units_and_zero_based_lines() {
        //  Ě = 2 bytes / 1 unit, 💔 = 4 bytes / 2 units (surrogates).
        let text = "ab\nĚx💔y\nok\n";
        assert_eq!(position(text, 0), Position::new(0, 0));
        assert_eq!(position(text, 3), Position::new(1, 0)); // line 2 start
        assert_eq!(position(text, 5), Position::new(1, 1)); // after Ě
        assert_eq!(position(text, 6), Position::new(1, 2)); // after x
        assert_eq!(position(text, 10), Position::new(1, 4)); // after 💔
        assert_eq!(position(text, 11), Position::new(1, 5)); // after y
        assert_eq!(position(text, 12), Position::new(2, 0)); // line 3 start
        assert_eq!(position(text, 14), Position::new(2, 2)); // after "ok"
        // Past the end clamps; mid-char offsets clamp DOWN, never panic.
        assert_eq!(position(text, 999), position(text, text.len()));
        assert_eq!(position(text, 4), Position::new(1, 0)); // inside Ě
    }

    #[test]
    fn snippet_spans_map_against_the_buffer() {
        let text = "line one\nbad line here\nline three\n";
        // Engine style: snippet is the quoted SOURCE LINE, span the whole
        // of it → the range covers line index 1.
        let snippet = Snippet {
            name: "-".to_owned(),
            text: "bad line here".to_owned(),
            span: (0, 13),
        };
        assert_eq!(
            range(text, &snippet),
            Range::new(Position::new(1, 0), Position::new(1, 13))
        );
        // JSON style: snippet is the WHOLE text, span a byte point.
        let snippet = Snippet {
            name: String::new(),
            text: text.to_owned(),
            span: (9, 3),
        };
        assert_eq!(
            range(text, &snippet),
            Range::new(Position::new(1, 0), Position::new(1, 3))
        );
        // Unlocatable snippet (an !include-file error) → neutral 0:0.
        let snippet = Snippet {
            name: "other.dsl".to_owned(),
            text: "nowhere near".to_owned(),
            span: (0, 12),
        };
        assert_eq!(
            range(text, &snippet),
            Range::new(Position::new(0, 0), Position::new(0, 0))
        );
    }

    #[test]
    fn collection_caps_at_max_diagnostics() {
        // The cap is a plain truncate after mapping — pin the constant's
        // use site honestly.
        let issue = Issue::error(Code::new(3001), "repeat");
        let text = "x\n";
        let many: Vec<Diagnostic> = (0..MAX_DIAGNOSTICS + 50)
            .map(|_| to_diagnostic(text, &issue))
            .collect();
        assert_eq!(many.len(), MAX_DIAGNOSTICS + 50);
        let capped = &many[..MAX_DIAGNOSTICS.min(many.len())];
        assert_eq!(capped.len(), MAX_DIAGNOSTICS);
    }
}
