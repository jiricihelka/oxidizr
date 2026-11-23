//! miette rendering of CLI diagnostics.
//!
//! The diagnostic data type itself is `oxidizr_api::Issue` (pure, shared with
//! the headless pipeline since T0); this module is the ONLY miette consumer:
//! it renders issue batches with the graphical handler. `--no-color` /
//! `NO_COLOR` select the ASCII theme.

use std::fmt;

use miette::{
    Diagnostic, GraphicalReportHandler, GraphicalTheme, LabeledSpan, NamedSource, Severity,
    SourceSpan,
};
use oxidizr_diag::Severity as OxzSeverity;

pub use oxidizr_api::Issue;

/// Renders issue batches to stderr (miette graphical handler).
pub struct Renderer {
    color: bool,
    quiet: bool,
}

impl Renderer {
    /// `NO_COLOR` and `--no-color` both disable color.
    #[must_use]
    pub fn new(no_color: bool, quiet: bool) -> Self {
        Renderer {
            color: !no_color && std::env::var_os("NO_COLOR").is_none(),
            quiet,
        }
    }

    /// Non-error chatter (suppressed by `--quiet`).
    pub fn ok(&self, message: &str) {
        if !self.quiet {
            println!("{message}");
        }
    }

    /// Human-report line on STDERR (stdout may carry machine output — the
    /// JSON commands write; suppressed by `--quiet`).
    pub fn note(&self, message: &str) {
        if !self.quiet {
            eprintln!("{message}");
        }
    }

    /// Render all issues to stderr (color honored).
    pub fn issues(&self, issues: &[Issue]) {
        let handler = GraphicalReportHandler::new()
            .with_theme(if self.color {
                GraphicalTheme::unicode()
            } else {
                GraphicalTheme::ascii()
            })
            .with_context_lines(1);
        let mut sink = String::new();
        for issue in issues {
            // Rendering to a String cannot fail; batched write keeps stderr
            // writes atomic-ish for interleaving.
            let rendered = Rendered::new(issue);
            let _ = handler.render_report(&mut sink, &rendered);
        }
        eprint!("{sink}");
    }
}

/// miette facade over an [`Issue`]: owns the `NamedSource` built from the
/// pure `oxidizr_api::Snippet` (miette types never cross the API boundary,
/// so the named source is materialized only at render time).
#[derive(Debug)]
struct Rendered<'a> {
    issue: &'a Issue,
    named: Option<NamedSource<String>>,
}

impl<'a> Rendered<'a> {
    fn new(issue: &'a Issue) -> Self {
        Rendered {
            issue,
            named: issue
                .snippet
                .as_ref()
                .map(|s| NamedSource::new(s.name.clone(), s.text.clone())),
        }
    }
}

impl fmt::Display for Rendered<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.issue.message.fmt(f)
    }
}

impl std::error::Error for Rendered<'_> {}

impl Diagnostic for Rendered<'_> {
    fn code(&self) -> Option<Box<dyn fmt::Display + '_>> {
        Some(Box::new(self.issue.code.to_string()))
    }

    fn severity(&self) -> Option<Severity> {
        Some(match self.issue.severity {
            OxzSeverity::Error => Severity::Error,
            OxzSeverity::Warning => Severity::Warning,
            OxzSeverity::Info => Severity::Advice,
        })
    }

    fn help(&self) -> Option<Box<dyn fmt::Display + '_>> {
        self.issue
            .help
            .as_ref()
            .map(|h| Box::new(h.clone()) as Box<dyn fmt::Display + '_>)
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        self.named.as_ref().map(|n| n as &dyn miette::SourceCode)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        self.issue.snippet.as_ref().map(|s| {
            let (offset, length) = s.span;
            let span = SourceSpan::from(offset..offset.saturating_add(length));
            Box::new(std::iter::once(LabeledSpan::new_primary_with_span(
                None, span,
            ))) as Box<dyn Iterator<Item = LabeledSpan> + '_>
        })
    }
}
