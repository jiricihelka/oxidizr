//! Fuzz the query filter grammar (§P4T T-5, gap 3): `oxidizr_dsl::query::run`
//! is the pinned `ExpressionParser` port over Model scope — a THROWING arm
//! must surface as a `DslError` VALUE, never a panic, whatever the filter
//! bytes are. The workspace is fixed (parsed once, lazily); only the
//! filter is the fuzzed surface. Seeds: `fuzz/seeds/query_parse/` (the
//! probe filter table).

#![no_main]

use libfuzzer_sys::fuzz_target;
use oxidizr_dsl::{
    engine::{ParseOptions, parse_dsl},
    query::run,
};

/// The fixed Model: identifiers, types and a view for the documented
/// view-subset arms to find.
const FIXTURE: &str = "workspace \"F\" \"\" {\n  model {\n    p = person \"P\" \"d\"\n    s = softwareSystem \"S\" {\n      c = container \"C\" \"\" \"PostgreSQL\"\n    }\n    p -> s \"uses\"\n  }\n  views {\n    systemLandscape \"l\" \"L\"\n  }\n}\n";

fuzz_target!(|data: &[u8]| {
    static WORKSPACE: std::sync::OnceLock<Option<oxidizr_core::Workspace>> =
        std::sync::OnceLock::new();
    let workspace = WORKSPACE
        .get_or_init(|| {
            parse_dsl(
                FIXTURE,
                ParseOptions {
                    file: None,
                    base_dir: None,
                    json_loader: None,
                    env_lookup: None,
                    installed_themes: None,
                },
            )
            .map(|r| r.workspace)
            .ok()
        })
        .as_ref()
        .expect("the fixed fixture must parse — its breakage is a bug, not fuzz noise");
    let filter = String::from_utf8_lossy(data);
    // The error is a value; reaching it (or the result) is success.
    let _ = run(workspace, &filter);
});
