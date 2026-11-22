//! OXZ-code coverage gate (P1T-D4): every registered diagnostic code the
//! pipeline can emit must appear in ≥1 corpus manifest, unless an explicit
//! allow-list entry registers WHY it is unreachable through the corpus. The
//! catalog — not strings — is the contract. Allow-list entries for codes the
//! growing corpus should reach are debt: they must shrink as milestones
//! land (T3 completes the invalid corpus; a covered code left on the list
//! fails this test, so the list cannot silently rot).
//!
//! §P4T gap 2: the registry is no longer a hand-copied list here — it IS
//! [`oxidizr_diag::codes::ALL`], and `registry_matches_docs_table` checks
//! that catalog against `docs/diagnostics.md` mechanically, so a code added
//! to the crate (like 5010 at O-4) can never again sit unregistered.
#![allow(clippy::unwrap_used)]

use oxidizr_diag::codes;
use oxidizr_e2e::corpus;

/// The full registry — the crate's own machine-readable catalog (mirrors
/// `docs/diagnostics.md`, kept in sync by `registry_matches_docs_table`;
/// 5003 and 5004 are tombstones and must never appear).
const REGISTRY: &[(u16, &str)] = codes::ALL;

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
        5007,
        "export not-implemented: since the WSD leg landed (P4 §P4-M2 N-5) only \
         `static` — the JVM site generator, out of oxidizr scope — raises it, and \
         the export matrix observes the four diagram formats only, so no manifest \
         can carry the code — permanent",
    ),
    (
        5009,
        "export abort (M2, EXPORT-PLANTUML.md §14) needs a corrupt-diagram \
         workspace (dangling id / view-kind cast mismatch) through a diagram \
         export — PT2 probed all three shapes on JSON feeds (dangling elementView \
         id, empty dynamic, kind-mismatched container/component view targets): \
         the load-time hydrate/validate gate rejects them as OXZ-4001 before \
         dispatch, and the DSL path mints views only from resolved symbols — \
         permanent (PHASE_2T.md)",
    ),
    (
        9001,
        "unreachable via the corpus: the Collector cap (oxidizr-diag, CAP=100 + overflow \
         note) is not wired into the pipeline — 120 broken lines emit 120 errors, no note \
         (T3 probed); register the wiring decision before any fixture can cover it",
    ),
    (
        5010,
        "generate failure (§P4-M3 O-4, P4-D9): the corpus runner has no `generate` leg — \
         the command takes a directory of workspaces plus `--include`/`--exclude` filters, \
         while the manifest schema fans out per single-workspace command (P4T-V1: verified \
         2026-11-22, the v2 sidecar has no section that can carry it) — T-3 \
         (PHASE_4T.md gap 4) adds the generate axes; until then this is registered debt, \
         and `allowlist_has_no_stale_entries` fails once a manifest covers the code",
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

/// §P4T gap 2 machine check: `codes::ALL` (the crate catalog the whole gate
/// reads) and the human table in `docs/diagnostics.md` must list exactly the
/// same LIVE codes — tombstone rows (severity `—`) excluded on both sides.
/// Catches the O-4 drift shape for good: a const or docs row added without
/// the other side, or without the catalog entry the coverage rows key on.
#[test]
fn registry_matches_docs_table() {
    let text =
        std::fs::read_to_string(oxidizr_e2e::repo_root().join("docs").join("diagnostics.md"))
            .expect("docs/diagnostics.md is the registry's target document");
    let mut documented: Vec<u16> = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("| `OXZ-") else {
            continue;
        };
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        // Exactly four digits: the range table (`OXZ-1xxx`) must not parse.
        let code: u16 = if digits.len() == 4 {
            digits.parse().expect("four digits parse")
        } else {
            continue;
        };
        // Registered rows carry a severity in column 2; tombstones carry `—`.
        let severity = line.split('|').nth(2).unwrap_or("").trim();
        if severity != "—" {
            documented.push(code);
        }
    }
    let mut catalog: Vec<u16> = REGISTRY.iter().map(|(c, _)| *c).collect();
    catalog.sort_unstable();
    documented.sort_unstable();
    assert_eq!(
        catalog, documented,
        "codes::ALL and docs/diagnostics.md disagree (tombstones excluded)"
    );
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
