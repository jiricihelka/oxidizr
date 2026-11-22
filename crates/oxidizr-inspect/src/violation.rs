//! `Violation` (`structurizr-inspection/Violation.java`) — one finding:
//! the inspection TYPE, its resolved [`Severity`], and the message.
//!
//! The reference carries the `Inspection` object and rewrites severity in
//! place (`withSeverity`); here it arrives fully resolved. The human report
//! line is the reference `toString()`: `type | SEVERITY | message`.

use std::fmt;

use crate::severity::Severity;

/// One inspection violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// `Inspection.getType()` — the dotted path, e.g.
    /// `model.container.technology` (the property-key stem and the
    /// `--only` / `--except` filter value).
    pub inspection: &'static str,
    /// Resolved severity (property strategy, see `crate::strategy`).
    pub severity: Severity,
    /// The reference message, verbatim.
    pub message: String,
}

impl fmt::Display for Violation {
    /// `Violation.toString()`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} | {} | {}",
            self.inspection,
            self.severity.java_name(),
            self.message
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_matches_the_java_to_string() {
        let v = Violation {
            inspection: "model.empty",
            severity: Severity::Warning,
            message: "The model is empty.".into(),
        };
        assert_eq!(v.to_string(), "model.empty | WARNING | The model is empty.");
    }
}
