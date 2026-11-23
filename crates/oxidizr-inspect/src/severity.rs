//! `Severity` — four levels, ranked `ERROR > WARNING > INFO > IGNORE`
//! for the CLI threshold filter.
//!
//! There are no per-inspection defaults: EVERY inspection starts as
//! [`Severity::Error`] and is only re-ranked through
//! `structurizr.inspection.*` properties.

use std::fmt;

/// One inspection severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Report and (CLI) fail the gate.
    Error,
    /// Report; fails the gate only at `--severity-threshold warning`.
    Warning,
    /// Report; informational.
    Info,
    /// Suppressed — never reported by `oxidizr inspect` (the
    /// official tooling only counts these in
    /// `structurizr.inspection.ignore`).
    Ignore,
}

impl Severity {
    /// The property-value grammar: `WARNING` / `INFO` / `IGNORE` /
    /// `ERROR`, case-insensitive. Documented deviation: the official
    /// tooling THROWS on an unparseable value; errors-as-values maps it
    /// to `Error` — the fail-loud direction a CI gate wants anyway.
    #[must_use]
    pub fn parse(value: &str) -> Severity {
        match value.to_ascii_uppercase().as_str() {
            "WARNING" => Severity::Warning,
            "INFO" => Severity::Info,
            "IGNORE" => Severity::Ignore,
            // "ERROR" and every unparseable value.
            _ => Severity::Error,
        }
    }

    /// Lowercase name (the JSON format's spelling).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
            Severity::Ignore => "ignore",
        }
    }

    /// The uppercase name (the human report line's spelling —
    /// `type | SEVERITY | message` prints the severity this way).
    #[must_use]
    pub fn java_name(self) -> &'static str {
        match self {
            Severity::Error => "ERROR",
            Severity::Warning => "WARNING",
            Severity::Info => "INFO",
            Severity::Ignore => "IGNORE",
        }
    }

    /// Gate rank: a violation is REPORTED iff its rank is >= the
    /// threshold's (`error` fails the CI gate at every threshold).
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Severity::Error => 3,
            Severity::Warning => 2,
            Severity::Info => 1,
            Severity::Ignore => 0,
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// CLI `--severity-threshold <level>` parsing (the three REPORTABLE
/// levels; `ignore` is not a threshold — nothing is above it).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid severity level `{0}` (expected: info, warning, error)")]
pub struct ParseThresholdError(pub String);

impl Severity {
    /// Parse a threshold token (case-insensitive).
    ///
    /// # Errors
    ///
    /// [`ParseThresholdError`] unless the token is one of `info`,
    /// `warning`, `error` (case-insensitive).
    pub fn parse_threshold(value: &str) -> Result<Severity, ParseThresholdError> {
        match value.to_ascii_lowercase().as_str() {
            "error" => Ok(Severity::Error),
            "warning" => Ok(Severity::Warning),
            "info" => Ok(Severity::Info),
            _ => Err(ParseThresholdError(value.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_parse_is_case_insensitive_with_error_fallback() {
        assert_eq!(Severity::parse("warning"), Severity::Warning);
        assert_eq!(Severity::parse("INFO"), Severity::Info);
        assert_eq!(Severity::parse("Ignore"), Severity::Ignore);
        assert_eq!(Severity::parse("error"), Severity::Error);
        // The official tooling throws on these; we fail loud (module docs).
        assert_eq!(Severity::parse("critical"), Severity::Error);
        assert_eq!(Severity::parse(""), Severity::Error);
    }

    #[test]
    fn ranking_orders_error_above_ignore() {
        assert!(Severity::Error.rank() > Severity::Warning.rank());
        assert!(Severity::Warning.rank() > Severity::Info.rank());
        assert!(Severity::Info.rank() > Severity::Ignore.rank());
    }

    #[test]
    fn threshold_parse() {
        assert_eq!(Severity::parse_threshold("error"), Ok(Severity::Error));
        assert_eq!(Severity::parse_threshold("WARNING"), Ok(Severity::Warning));
        assert_eq!(Severity::parse_threshold("Info"), Ok(Severity::Info));
        assert!(Severity::parse_threshold("ignore").is_err());
        assert!(Severity::parse_threshold("nope").is_err());
    }
}
