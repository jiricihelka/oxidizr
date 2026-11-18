//! OXZ-code coverage gate (P1T-D4): every registered diagnostic code the
//! pipeline can emit must appear in ≥1 corpus manifest, unless an explicit
//! allow-list entry registers WHY it is unreachable through the corpus. The
//! catalog — not strings — is the contract. Allow-list entries for codes the
//! growing corpus should reach are debt: they must shrink as milestones
//! land (T3 completes the invalid corpus; a covered code left on the list
//! fails this test, so the list cannot silently rot).
#![allow(clippy::unwrap_used)]

use oxidizr_e2e::corpus;

/// The full registry (`oxidizr_diag::codes`, mirrors docs/diagnostics.md;
/// 5004 is a tombstone and must never appear).
const REGISTRY: &[(u16, &str)] = &[
    (1001, "LEXER_UNTERMINATED_QUOTED"),
    (2001, "UNEXPECTED_TOKEN"),
    (2002, "STATEMENT"),
    (2003, "UNEXPECTED_EOF"),
    (3001, "VALIDATION"),
    (3002, "RECONCILE"),
    (4001, "JSON_READ"),
    (4002, "JSON_WRITE"),
    (5001, "NET_DISABLED"),
    (5002, "JVM_ONLY"),
    (5003, "DOCS_DEFERRED"),
    (5005, "IMPORT_DEFERRED"),
    (5006, "CLI_IO"),
    (5007, "EXPORT_NOT_IMPLEMENTED"),
    (5008, "EXPORT_THEME"),
    (5009, "EXPORT_ABORT"),
    (9001, "TOO_MANY_ERRORS"),
];

/// Codes no manifest covers, each with its registered reason.
const ALLOWLIST: &[(u16, &str)] = &[
    (
        1001,
        "bug-compat: never emitted by default (reserved for --strict linting) — permanent",
    ),
    (
        4002,
        "unreachable via the corpus: every workspace shape the writer refuses (filtered \
         view without base, dangling view links) is rejected earlier by the JSON hydrate \
         gate as OXZ-4001, and DSL parsing cannot build one — T3 researched",
    ),
    (
        5008,
        "theme loading (P2-D3) landed at M1; the 2T export matrix runs themes on \
         every applicable entry (5001 fires for URL themes offline) — 5008 needs a \
         broken theme FILE, the 2T fixtures add one; remove when covered \
         (PHASE_2T.md)",
    ),
    (
        5009,
        "export abort (M2, EXPORT-PLANTUML.md §14) needs a corrupt-diagram \
         workspace (dangling id / view-kind cast mismatch) through a diagram \
         export — the hydrate gate blocks such fixtures at read (T3-era probe); \
         2T probes the reachable shapes, else this stays permanent \
         (PHASE_2T.md)",
    ),
    (
        9001,
        "unreachable via the corpus: the Collector cap (oxidizr-diag, CAP=100 + overflow \
         note) is not wired into the pipeline — 120 broken lines emit 120 errors, no note \
         (T3 probed); register the wiring decision before any fixture can cover it",
    ),
];

fn manifest_codes() -> Vec<u16> {
    let entries = corpus::discover().expect("corpus discovers");
    let mut codes: Vec<u16> = Vec::new();
    for entry in &entries {
        let sidecar = entry.dir.join(oxidizr_e2e::manifest::FILE_NAME);
        let manifest = oxidizr_e2e::manifest::Manifest::read(&sidecar)
            .unwrap_or_else(|e| panic!("sidecar unreadable: {e}"));
        for report in [
            manifest.dsl2json,
            manifest.json2json,
            manifest.validate,
            manifest.reconcile,
        ]
        .into_iter()
        .flatten()
        {
            codes.extend(report.codes);
        }
        if let Some(exports) = &manifest.export {
            for (_, report) in exports.formats() {
                codes.extend(report.codes.iter().copied());
            }
        }
    }
    codes.sort_unstable();
    codes.dedup();
    codes
}

#[test]
fn every_registry_code_is_covered_or_registered() {
    let covered = manifest_codes();
    let gaps: Vec<String> = REGISTRY
        .iter()
        .filter(|(code, _)| !covered.contains(code))
        .filter(|(code, _)| !ALLOWLIST.iter().any(|(c, _)| c == code))
        .map(|(code, name)| {
            format!("OXZ-{code:04} {name}: not covered by any manifest and not allow-listed")
        })
        .collect();
    assert!(gaps.is_empty(), "code coverage gaps:\n{}", gaps.join("\n"));
}

#[test]
fn allowlist_has_no_stale_entries() {
    let covered = manifest_codes();
    let stale: Vec<&(u16, &str)> = ALLOWLIST
        .iter()
        .filter(|(code, _)| covered.contains(code))
        .collect();
    assert!(
        stale.is_empty(),
        "allow-listed codes are now covered — shrink the list: {stale:?}"
    );
}

#[test]
fn manifests_only_use_registered_codes() {
    let known: Vec<u16> = REGISTRY.iter().map(|(c, _)| *c).collect();
    for code in manifest_codes() {
        assert!(
            known.contains(&code),
            "manifest carries OXZ-{code:04}, which is not in the registry — \
             register it in docs/diagnostics.md and here first"
        );
    }
}
