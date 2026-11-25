//! `initializationOptions` — the whole per-connection configuration,
//! parsed ONCE at `initialize` from the client's
//! `initializationOptions` JSON:
//!
//! ```json
//! { "strict": false, "themesDir": null, "allowNet": false,
//!   "maxNumberOfProblems": 100 }
//! ```
//!
//! `strict`/`themesDir`/`allowNet` are the CLI flags of the same names
//! ([`oxidizr_api::ApiOptions`]) and ride EVERY pipeline
//! run the server performs, so the editor sees exactly what `oxidizr
//! validate` would see with those flags. `allowNet` defaults to `false`
//! and the LSP NEVER introduces network access of its own — the flag
//! only lifts the pipeline's own theme-URL gate. `maxNumberOfProblems`
//! caps every `publishDiagnostics` (default
//! `crate::diagnostics::MAX_DIAGNOSTICS`).
//!
//! Parsing is TOTAL on purpose — hand-rolled over [`serde_json::Value`]
//! (no serde-derive dependency for four fields), each field read
//! independently and IGNORED when absent or of the wrong type, unknown
//! keys dropped: a broken options blob answers the DEFAULTS and can
//! never fail `initialize` or cost the user a language feature.

use std::path::PathBuf;

use oxidizr_api::{ApiOptions, ParseFlags};
use serde_json::Value;

use crate::diagnostics::MAX_DIAGNOSTICS;

/// The parsed `initializationOptions` (defaults = the CLI's own defaults
/// plus the 100-problem editor cap).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// `--strict`: warnings block (exit 1) — passed to the pipeline.
    pub strict: bool,
    /// `--themes-dir <dir>`: highest-priority installed-theme source.
    pub themes_dir: Option<PathBuf>,
    /// `--allow-net`: lifts the pipeline's theme-URL fetch gate.
    /// OFF by default; the LSP itself never touches the network.
    pub allow_net: bool,
    /// `maxNumberOfProblems`: the `publishDiagnostics` cap (0 is
    /// honored literally — silence is a valid client choice).
    pub max_problems: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            strict: false,
            themes_dir: None,
            allow_net: false,
            // NOT the derived zero: silence is a choice the client makes
            // with an explicit `maxNumberOfProblems: 0`.
            max_problems: MAX_DIAGNOSTICS,
        }
    }
}

impl Config {
    /// Read the client's `initializationOptions` value — field by field,
    /// leniently ([`module`][self] docs). A non-object value (or `None`)
    /// is simply "no options".
    #[must_use]
    pub fn from_options(options: Option<&Value>) -> Self {
        let mut config = Config::default();
        // `as_u64` rejects negatives and fractions; a 64-bit-max value
        // clamps to `usize::MAX` (never wraps).
        if let Some(value) = options.and_then(Value::as_object) {
            if let Some(strict) = value.get("strict").and_then(Value::as_bool) {
                config.strict = strict;
            }
            if let Some(dir) = value.get("themesDir").and_then(Value::as_str) {
                config.themes_dir = Some(PathBuf::from(dir));
            }
            if let Some(net) = value.get("allowNet").and_then(Value::as_bool) {
                config.allow_net = net;
            }
            if let Some(max) = value.get("maxNumberOfProblems").and_then(Value::as_u64) {
                config.max_problems = usize::try_from(max).unwrap_or(usize::MAX);
            }
        }
        config
    }

    /// The pipeline flags every `api::open`/last-good parse uses.
    #[must_use]
    pub fn api_options(&self) -> ApiOptions {
        ApiOptions {
            parse: ParseFlags {
                strict: self.strict,
                ..ParseFlags::default()
            },
            themes_dir: self.themes_dir.clone(),
            allow_net: self.allow_net,
            ..ApiOptions::default()
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn options(text: &str) -> Value {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn absent_options_are_the_defaults() {
        let blobs = [options("null"), options("42"), options(r#""s""#)];
        let cases = [None].into_iter().chain(blobs.iter().map(Some));
        for blob in cases {
            let config = Config::from_options(blob);
            assert_eq!(config, Config::default());
            assert_eq!(config.max_problems, MAX_DIAGNOSTICS);
            assert!(!config.allow_net, "network is off without allowNet (D5)");
        }
    }

    #[test]
    fn camel_case_keys_parse() {
        let config = Config::from_options(Some(&options(
            r#"{"strict": true, "themesDir": "/opt/themes",
               "allowNet": true, "maxNumberOfProblems": 7}"#,
        )));
        assert!(config.strict);
        assert_eq!(config.themes_dir, Some(PathBuf::from("/opt/themes")));
        assert!(config.allow_net);
        assert_eq!(config.max_problems, 7);
    }

    #[test]
    fn fields_read_independently() {
        // The wrong-typed and unknown fields drop; the sound field lands.
        let config = Config::from_options(Some(&options(
            r#"{"strict": "yes", "nope": 1, "maxNumberOfProblems": 5}"#,
        )));
        assert!(!config.strict, "a string is not a bool — default stands");
        assert_eq!(config.max_problems, 5);
        assert_eq!(config.themes_dir, None);
        // 0 is a choice, negatives/fractions are not numbers-as-u64.
        let zero = Config::from_options(Some(&options(r#"{"maxNumberOfProblems": 0}"#)));
        assert_eq!(zero.max_problems, 0, "0 silences the publish");
        let junk = Config::from_options(Some(&options(r#"{"maxNumberOfProblems": -3}"#)));
        assert_eq!(junk.max_problems, MAX_DIAGNOSTICS);
    }

    #[test]
    fn api_options_ride_the_pipeline() {
        let config = Config::from_options(Some(&options(
            r#"{"strict": true, "themesDir": "/t", "allowNet": true}"#,
        )));
        let opts = config.api_options();
        assert!(opts.parse.strict);
        assert!(opts.allow_net);
        assert_eq!(opts.themes_dir, Some(PathBuf::from("/t")));
        // The untouched flags stay the production defaults.
        assert!(!opts.parse.keep_unknown);
        assert!(opts.env.is_none());
    }
}
