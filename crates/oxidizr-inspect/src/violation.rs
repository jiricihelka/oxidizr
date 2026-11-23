//! `Violation` — one finding: the inspection TYPE, its resolved
//! [`Severity`], and the message.
//!
//! A violation arrives fully resolved (severity is never rewritten in
//! place). The human report line is `type | SEVERITY | message`.

use std::fmt;

use crate::severity::Severity;

/// One inspection violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The inspection type — the dotted path, e.g.
    /// `model.container.technology` (the property-key stem and the
    /// `--only` / `--except` filter value).
    pub inspection: &'static str,
    /// Resolved severity (property strategy, see `crate::strategy`).
    pub severity: Severity,
    /// The official tooling's message, verbatim.
    pub message: String,
}

impl fmt::Display for Violation {
    /// The human report line: `type | SEVERITY | message`.
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
