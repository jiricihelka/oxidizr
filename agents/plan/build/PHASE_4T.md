# PHASE_4T.md — detailed plan: Phase 4 feature test coverage (interlude)

**Status: SCAFFOLDED 2026-11-22 (user directive) — not started.**
Position: after §P4-M5 items E-1…E-3 (the `oracle` command, the two P2-era
comparators, the `Layered` polish — tooling/quality pieces), **before** P4R
(which rides the net this phase weaves) and P4D. The release tail (§P4-M5
E-4/E-5: cargo-dist + `v0.4.0` tag + changelog) moves to the very end,
after P4D — user decision 2026-11-22.

Goal: every feature that landed since v0.3.0 — LSP, `diff`, `inspect`, the
diagram/`!docs`/`!adrs` importers, WSD export, `query`, `init`,
`completions`, `generate system-landscape`, the `oracle` user command — is
covered by at least one NAMED regression test in every suite that fits it,
and the coverage itself becomes a checked artifact. Payoff: safe future
updates, and the groundwork for the planned post-0.4 refactor of
Java-era mistakes (no feature may regress silently).

## P4T-D0 Charter (and the freeze)

- **Additive phase**: new tests, fixtures, matrix rows, gate fixes. A test
  that FINDS a bug is fixed at the cause (the fix ships in the same commit,
  register entry if it touches the reference contract); no golden or blessed
  sidecar may change because of P4T — a changed expectation is by
  definition a bug in the test or an undisclosed behavior change.
- `testdata/` vendored content is never hand-edited (its README contract);
  new fixtures are authored originals under `e2e/corpus` / `testdata/golden`
  subdirs the bless flow owns.
- One representative named test per matrix cell — the matrix is an
  accountability document, not a mandate to duplicate coverage.
- Exit: matrix complete with zero open gaps, quartet green, full e2e tier
  green + bless no-op, `cargo xtask fuzz` time-boxed legs clean, milestone
  log in PROGRESS. **No tag** (interlude precedent 1T/2T/2R).

## P4T-D1 The matrix (the deliverable)

A permanent table in `agents/plan/TESTING.md` §Feature × suite matrix:
rows = features (through Phase 4, P4T additions included), columns =
`unit` · `golden (Cat A)` · `CLI` · `e2e quick` · `e2e full` · `Cat B oracle`
· `fuzz`. Cells carry the TEST NAME (harvested from
`cargo test -- --list`, not memory) or a gap id resolved by a milestone
item. Maintenance rule: a commit that adds a feature row must add its
matrix row; P4D keeps the section prose-clean.

## P4T-D2 Known seed gaps (inventory of 2026-11-22)

1. `oxidizr-import` has NO `tests/` dir — importer coverage is unit-only.
   Add corpus-style golden probe files (C4-PlantUML, Mermaid, image →
   CustomView, `!docs`/`!adrs` per `IMPORT-DOCS-ADRS.md`) as a Category A
   golden file suite.
2. OXZ-code coverage gate (`oxidizr-e2e/tests/coverage.rs`) is stale since
   O-4: `5010 GENERATE` is missing from REGISTRY — re-sync the registry
   against `oxidizr_diag::codes` (machine check preferred: a test asserting
   the two lists agree beats a comment) and give 5010 a corpus manifest or
   a registered allow-list reason.
3. Fuzz targets are `dsl_parse`/`dsl_fmt`/`json_read` only — the P4 parsers
   are unfuzzed. Add targets + seed corpora: `import_sniff` (Mermaid/C4
   text), `query_parse`, `pathfilter` (the generate regex subset), and a
   `diff_pair` round-trip target (same workspace twice = no findings).
4. e2e corpus has no Phase 4 legs: extend manifests (schema v2 → v3 if the
   section shape needs new kinds — verify first, §P4T-V1) with `inspect`
   violation-count, `query` smoke and `diff a a`-identity axes in the full
   tier; `generate` merge invariants over paired fixtures (no views ever,
   id uniqueness across the merged set).
5. LSP: one happy-path transcript only. Add per-capability transcripts
   (completion, hover, formatting, workspace-edit idempotence) and the
   P4-V5 fast-edit stress carry-in (rapid didChange bursts → final
   diagnostics converge).
6. CLI: audit row per `Command` — each subcommand must own ≥ 1 stdout/stderr
   golden + ≥ 1 error-path exit-code test (the `oracle` command arrives
   with its own suite in E-1; the matrix only records it).
7. Category B: the E-2 plantuml/mermaid comparators run over the FULL C4 +
   Mermaid golden fixture set (same leg shape as the C4 comparator), and
   `oracle verify` stays the close ritual leg.

## P4T-D3 Safety net during the phase

Every commit: quartet (`cargo xtask ci`). Every wave: full tier + bless
no-op (2R rule — zero git diff after bless). Fuzz: `cargo xtask fuzz`
time-boxed runs at wave close (nightly; findings = bugs, fixed at the
cause, seeds added to the corpus). Category B (`oracle verify`, extended
`export-verify`, `oracle landscape`) once at phase close.

## P4T-D4 Milestones (ordered, each ends green — quartet + commit + RESUME)

| Item | Scope | Commit |
|---|---|---|
| **T-1** | The matrix section + gate re-sync (gap 2) + harvest script notes | **T1b DONE** (gate: `codes::ALL` + docs↔catalog machine check + 5010 registered debt); T1a (matrix section) pending |
| **T-2** | `oxidizr-import` golden file suite (gap 1) | T2 |
| **T-3** | e2e corpus Phase-4 axes + invariants (gap 4) | T3 |
| **T-4** | LSP capability transcripts + fast-edit stress (gap 5) | T4 |
| **T-5** | New fuzz targets + seed corpora (gap 3) | T5 |
| **T-6** | CLI audit row (gap 6), Category B extension (gap 7), close ritual (full tier + bless + PROGRESS `P4T` log + matrix frozen) | T6 |

## P4T-D5 Verification list (empirical, before dependent code)

- P4T-V1 — e2e manifest schema: can v2 sections carry inspect/query/diff
  legs, or does the fan-out need a v3? Read the manifest parser before
  authoring fixtures. **VERIFIED 2026-11-22 (T1b research): v2 cannot —
  `manifest.rs` has a FIXED section set (dsl2json/json2json/validate/
  reconcile/export/fmt) and unknown top-level keys FAIL the read (loud by
  design); there is no schema-version field, so extending the section set
  IS the v3. T-3 adds optional sections (`generate` needs multi-input +
  filter config the v2 shape cannot carry; inspect/query/diff legs need
  their count-asserts).**
- P4T-V2 — fuzz target cost/benefit on the P4 parsers (does `pathfilter`
  fuzz find anything real given it rejects non-supported regex up front?).
- P4T-V3 — LSP stress: determinism of the converge check under
  `tower-... ` test harness timing (keep it wall-clock-free; use the
  existing transcript harness).

## Risks

- **Matrix bloat** → one named test per cell rule (D0); the harvest is
  scripted, not hand-counted.
- **Bugs found late** (T-3/T-5 exposing real defects) → charter already
  admits cause-level fixes; DEVIATIONS/PROGRESS records each; the close
  bless no-op re-proves the contract after the fixes.
- **Fuzz rabbit holes** → time-boxed legs (the existing xtask fuzz job is
  the budget), findings beyond the day get a note + a deferred issue row.
