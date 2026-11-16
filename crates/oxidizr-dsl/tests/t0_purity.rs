//! Phase 1T V-T1 (`agents/plan/build/PHASE_1T.md`): the engine is usable for
//! parallel in-process runs — no global state beyond the benign UUID-uniqueness
//! counter, so concurrent parses of the same sources are deterministic in
//! every non-volatile aspect (the per-parse `structurizr.dsl.identifier` UUIDs
//! and the process env are excluded by design: V-3 / the e2e volatile filter).
#![allow(clippy::unwrap_used)]

use oxidizr_dsl::engine::{EnvLookup, ParseOptions, parse_dsl};

/// Hermetic env lookup (Phase 1T P1T-D1): nothing resolves, process env is
/// never consulted.
fn no_env(_name: &str) -> Option<String> {
    None
}

fn opts() -> ParseOptions<'static> {
    ParseOptions {
        file: None,
        base_dir: None,
        json_loader: None,
        env_lookup: Some(EnvLookup::new(no_env)),
    }
}

/// Non-volatile parse fingerprint: counts + stable error shape.
#[derive(Debug, PartialEq, Eq)]
struct Fingerprint {
    elements: usize,
    relationships: usize,
    views: usize,
    errors: Vec<(u16, String)>,
}

fn fingerprint(source: &str) -> Fingerprint {
    // None of SOURCES hits the fatal empty-source Err arm.
    let r = parse_dsl(source, opts()).unwrap();
    let model = &r.workspace.model;
    Fingerprint {
        elements: model.people.len()
            + model.software_systems.len()
            + model
                .software_systems
                .iter()
                .map(|s| s.containers.len())
                .sum::<usize>(),
        relationships: model.relationships.len(),
        views: r.workspace.views.custom_views.len()
            + r.workspace.views.system_landscape_views.len()
            + r.workspace.views.system_context_views.len()
            + r.workspace.views.container_views.len()
            + r.workspace.views.component_views.len()
            + r.workspace.views.dynamic_views.len()
            + r.workspace.views.filtered_views.len()
            + r.workspace.views.deployment_views.len(),
        errors: r
            .errors
            .iter()
            .map(|e| (e.code.get(), e.message.clone()))
            .collect(),
    }
}

const SOURCES: &[&str] = &[
    // clean multi-feature fragment
    "workspace \"P\" {\n    model {\n        a = person \"A\"\n        sys = softwareSystem \"S\" {\n            c = container \"C\"\n        }\n        a -> sys \"uses\"\n    }\n    views {\n        systemLandscape \"l\" {\n            include *\n        }\n    }\n}\n",
    // env-dependent text resolved hermetically: unresolved `${…}` stays raw
    "workspace \"E\" {\n    model {\n        s = softwareSystem \"${OXZ_MISSING}\" { properties \"k\" \"${OXZ_MISSING}-v\" }\n    }\n}\n",
    // broken-but-recoverable (the D1 multi-error shape must be stable too)
    "workspace \"X\" {\n    model {\n        s = softwareSystem\n    }\n}\n",
];

#[test]
fn concurrent_parses_are_deterministic() {
    let expected: Vec<Fingerprint> = SOURCES.iter().map(|s| fingerprint(s)).collect();
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                for (i, source) in SOURCES.iter().enumerate() {
                    assert_eq!(
                        fingerprint(source),
                        expected[i],
                        "concurrent fingerprint drift on source {i}"
                    );
                }
            });
        }
    });
}
