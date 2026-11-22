# PHASE_4T.md — detailed plan: Phase 4 feature test coverage (interlude)

**Status: CLOSED 2026-11-22 — T-1…T-6 all DONE, close ritual green (full e2e
tier · bless 307×unchanged · oracle verify 4+4+99+6 · export-verify 31/31 × 3 ·
landscape 10 facts · matrix FROZEN); no tag (interlude precedent).**
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
| **T-1** | The matrix section + gate re-sync (gap 2) + harvest script notes | **DONE** — T1a: `TESTING.md` §Feature × suite matrix (names harvested from `cargo test --workspace -- --list`, gap legend T-2…T-6); T1b: gate re-sync (`codes::ALL` + docs↔catalog machine check + 5010 registered debt) |
| **T-2** | `oxidizr-import` golden file suite (gap 1) | **DONE** — `crates/oxidizr-import/tests/golden_files.rs` (9 tests): 11 authored fixtures under `tests/fixtures/` (LF-pinned, inventory-pinned by `every_fixture_is_consumed`), digests blessed via UPDATE_EXPECT; MADR 0001 carries a front-matter `date:` because the importer's fallback is file **mtime** (an mtime-derived golden would be nondeterministic — proven mtime-proof by re-run after `touch`) |
| **T-3** | e2e corpus Phase-4 axes + invariants (gap 4) | **DONE** — manifest schema v3: `inspect`/`query`/`diff` axes over the entry's own input + `generate` over the entry directory (P4T-V1 settled); 307 sidecars re-blessed; OXZ-5010 entered the corpus, allow-list shrank (the stale-entry tripwire fired as designed); full tier adds `generate_pair_merge_is_view_free_and_id_unique` (view-free · unique ids · one edge per pair under `First`) |
| **T-4** | LSP capability transcripts + fast-edit stress (gap 5) | **DONE** — `tests/transcript.rs` +4 scripted exchanges over the real `run` loop: completion / hover (identifier-card · keyword-card · null) / formatting-**idempotence** (messy → ONE edit → apply → 0 edits) / `fast_edit_burst_converges` (open + 8 queued didChanges, LAST publish = LAST version = LAST buffer's truth). Summarizer grew per-shape lines (completion `n [first 3]`, `edits=n`, hover first markdown line) |
| **T-5** | New fuzz targets + seed corpora (gap 3) | **DONE** — 4 targets (`import_sniff`/`query_parse`/`pathfilter`/`diff_pair`), 28 committed seeds, `FUZZ_TARGETS` staged to 7; the battery found TWO REAL engine bugs, both fixed code-side: `48aa18b` fmt BOM idempotence (line-START protective space) and `0e8d7f3` pathfilter zero-width doubling (`$+$+…` was 2^n — each continuation position now tried exactly once, JDK-witnessed table). Slow-unit backtracking = PARITY with `java.util.regex` → surface bounded (fixed ≤4-char subjects), never a matcher cap. Finds note: `testing/fuzz-p4t-finds.md` |
| **T-6** | CLI audit row (gap 6), Category B extension (gap 7), close ritual (full tier + bless + PROGRESS `P4T` log + matrix frozen) | **DONE** — gap 6: the audit found `export` the ONLY `Command` without CLI subprocess tests; 4 legs in `tests/cli.rs` (`export_plantuml_writes_view_and_legend_files`, `export_notes_are_golden_and_exit_zero` — the two reference INFO notes byte-exact, both exit 0 —, `export_exit_family_static_png_missing_usage` — `static`/`png` OXZ-5007 exit 1, the reference AGREES for PNG/SVG (exit 1, witnessed `TZ=UTC`); `static` stays a registered known-but-unlanded exporter —, `export_websequencediagrams_writes_the_dynamic_view_file`), `Command::Export` doc comment refreshed (png/svg are KNOWN OXZ-5007 formats, not unknown exit-0). Gap 7: already DELIVERED at §P4-M5 E-2 — `cmd_export_verify --format c4\|plantuml\|mermaid` runs the semantic comparators over the full per-dialect golden dirs (31 fixtures + bulk-operations); recorded here, RUN at close. Close ritual: full e2e tier green · bless 307×unchanged + `testdata/` clean · `oracle verify` 4 json + 4 sidecar + 99 diagram + 6 theme · `export-verify` 31/31 × 3 · `oracle landscape` 10 facts (all `TZ=UTC`) · PROGRESS `P4T` board row + milestone log · matrix FROZEN (zero `T-n` cells). **P4T CLOSED — no tag** (interlude precedent) |

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
  **VERIFIED at T-5 (2026-11-22): YES — the rejection gate is compile-side
  only, so `is_match` over accepted patterns was live and found a REAL
  engine bug (zero-width quantifier iteration doubled the continuation →
  `$+$+…` 2^n timeout, fixed in `0e8d7f3` with a JDK-probed semantics
  table); `dsl_fmt` independently found the BOM idempotence bug (`48aa18b`).
  The remaining slow-units are ambiguous-quantifier backtracking — parity
  with the reference engine (unbounded there too), answered by bounding the
  fuzz surface (fixed short subjects), NOT by capping the matcher.**
- P4T-V3 — LSP stress: determinism of the converge check under
  `tower-... ` test harness timing (keep it wall-clock-free; use the
  existing transcript harness). **VERIFIED at T-4 (2026-11-22): no
  `tower-test` involved — `server::run` is a single-threaded FIFO loop
  over the receiver, so the T-4 client scripts its FULL exchange before
  draining and message order is structural. `fast_edit_burst_converges`
  pins exactly that: 9 publishes in send order, LAST publish = LAST
  version = LAST buffer's truth; zero sleeps, zero clock reads.**

## Risks

- **Matrix bloat** → one named test per cell rule (D0); the harvest is
  scripted, not hand-counted.
- **Bugs found late** (T-3/T-5 exposing real defects) → charter already
  admits cause-level fixes; DEVIATIONS/PROGRESS records each; the close
  bless no-op re-proves the contract after the fixes.
- **Fuzz rabbit holes** → time-boxed legs (the existing xtask fuzz job is
  the budget), findings beyond the day get a note + a deferred issue row.
