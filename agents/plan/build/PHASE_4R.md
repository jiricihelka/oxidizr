# PHASE_4R.md — detailed plan: idiomatic-Rust refactor pass, API churn allowed (interlude)

**Status: SCAFFOLDED 2026-11-22 (user directive) — not started.**
Position: after P4T (it rides the freshly woven net), before P4D (docs are
written LAST, describing the final code). The release tail (§P4-M5 E-4/E-5)
follows P4D.

Goal: walk the Phase 3+4 code (and leftovers 2R never touched) doing
small-scope refactors that make it nicer and more "rusty" — and, unlike
2R, **the public Rust API may churn**: renames, module splits, typed
replacements of stringly APIs, parameter structs (user decision
2026-11-22, overriding the 2R API freeze for this phase only). **Churn
depth (user, same day): structural enum-shape reworks ARE in scope** —
e.g. reworking the giant `Request` payload per domain — each as its own
quartet-green commit. What stays frozen is the PRODUCT: byte outputs,
the CLI grammar, exit classes, OXZ codes, `testdata/`, blessed sidecars.

## P4R-D0 Charter and freeze scope (ratified 2026-11-22, user Q&A)

**In scope**: the 2R catalogue (accessor naming, needless `clone`, index
loops vs iterators, `Option` ladders, stringly-typed dispatch, out-params,
`pub` over-exposure, long-method decomposition, string-building idioms)
**plus** public-API ergonomics: crate surface review (`pub use` graphs),
`Request`/`Request`-adjacent enum shapes in `oxidizr-api`, the `generate`
`pathfilter` and `query` engine APIs, `oxidizr-inspect` type naming,
layout option structs, `oxidizr-lsp` server/config types, viewer shell
plumbing — each as ITS OWN small commit.

**Frozen (the product contract)** — same table as 2R §D0:
`oxidizr-api` `execute` semantics observable through the CLI, CLI argv
grammar + exit classes + stdout/stderr usage, ALL byte outputs (workspace
JSON, diagram bytes, OXZ codes), `testdata/` and blessed e2e sidecars.
A wave that changes one of these is red by definition — fixed at the
cause, never blessed.

**API-churn obligations** (the price of the freedom): every public-item
rename/move/removal lands in a `CHANGELOG.md` "Breaking-ish (pre-1.0)"
line item in the same commit (the v0.4.0 section accumulates them at the
release tail); P4T matrix cells cite test NAMES, so a test rename in a
wave updates the matrix row in the same commit; `cargo doc -p <crate>
--no-deps` stays warning-free after every wave (broken intra-doc links are
the drift detector — full rustdoc style is P4D's job, truthfulness is
ours).

**Out of scope**: dependency changes, error-taxonomy redesign, anything
DEVIATIONS.md Locked, perf *optimizations* beyond what a mechanical idiom
fix brings (claims need criterion evidence, WORKFLOW §Code quality),
behavioral "fixes" for Java-era mistakes (that is the future phase — P4R
keeps behavior bit-identical, `pathfilter`/comparator semantics
included).

## P4R-D1 Safety net (what must stay green, when)

- Every commit: quartet (`cargo xtask ci`).
- Every wave: full tier (`OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e`)
  + **bless no-op** (zero git diff after `cargo xtask e2e bless`) — with
  the P4T matrix this now guards the Phase 4 surfaces too.
- Hot-path waves (lexer, writers, export/layout engines, pathfilter):
  `cargo xtask bench cmp <baseline>`; > ~10 % swing reviewed before merge.
- Phase close: quartet + full tier + bless no-op + `cargo xtask oracle
  verify` + extended `export-verify` legs (Category B, once).

## P4R-D2 Catalog — seeds measured against the tree (R0 inventory 2026-11-22)

Seeds were hypotheses; R0 measured each (files, not memory). Verdict per
seed; `keep` = re-audited, the allow/comment is the answer, no churn.

- `Request` (`oxidizr-api/pipeline.rs:155`, **10 variants**) — payloads DO
  mirror argv shapes 1:1. R3 flagship: per-domain group per the churn
  depth decision; `execute` semantics + CLI mapping stay byte-identical.
- `generate.rs` merge engine — **seed CORRECTED: no `HashMap<String, …>`
  ladders exist** (0 matches); name lookups are `Vec` scans
  (`find_element` linear per merge input). Item shrinks to: index-by-name
  ONLY if a typed, behavior-identical reader reads better — otherwise a
  `keep` row. `pathfilter` public API shape: `compile`/`is_match` + the
  `Node` tree is already the surface; leave.
- `inspect` — **seed CORRECTED: 5 modules, not ~35**; `Inspection` names
  live in `lib.rs` (30 hits) + `run.rs` (24). Naming pass = the
  `<Type>Inspection` ↔ export-graph review only.
- importers — the `ImportTarget` enum + `resolve_importer(fqn)` registry is
  already typed; the stringly leg is the **cli-side extension sniffer**
  (`ImportCmdFormat` + extension match in `oxidizr-cli`). Item: move the
  sniff table into `oxidizr-import` as a typed constructor (API churn).
  `regexlite`/`pathfilter` share a regex-subset core? **Decision item, R2**
  (grammar shapes differ — full-match literal-ish vs Java subset; default
  answer is KEEP SPLIT + record).
- cli — `to_request` (`lib.rs:572`, 155 lines) carries a justified
  `too_many_lines` allow ("the argv matrix IS the content"); **`command_name`
  seed CORRECTED: it lives in `#[cfg(test)]`** (test helper, not product).
  Item: derive/table experiment is DECLINED — keep row, documented.
  `report` renderer plumbing: small review.
- allow audit (R0 ran it): 157 `allow(clippy::…)` lines / 88 files;
  non-test ~106, **every site carries a justification comment** (the ratchet
  is honest — spot list: engine arm-order `too_many_lines` ×7 "arm order IS
  the contract", `javahash`/`styles`/`structurizr.rs` casts = Java contract,
  `float_cmp` layout = exact-value-by-design). Wave-level task reduces to
  the 2026-22-era additions only: `struct_excessive_bools` ×3
  (`ApiOptions`, cli flags struct, viewer shell) — parameter-struct
  candidates, R3/R4.

## P4R-D3 Waves (each: quartet + full tier + bless no-op + commit)

| Wave | Scope (items from the D2 inventory) | Commit |
|---|---|---|
| **R0** | Inventory DONE 2026-11-22: allow audit (157 sites / 88 files, non-test ~106, EVERY site justified — ratchet honest, wave task reduced to the three `struct_excessive_bools`); `cargo doc --workspace --no-deps` baseline 43 warnings → **0** (private links → code spans, fn/mod ambiguities → `fn@`/`mod@`, cross-module paths → `crate::`); bench baseline `4T` saved + committed (104 files); D2 re-measured with three seed CORRECTIONS (generate has no HashMap ladders — `Vec` scans; `command_name` is test-only; inspect is 5 modules) | R0 |
| **R1** | core/ast/diag/dsl/json — **DONE 2026-11-22**: `create_default_views` decomposed into three named leg helpers, the `too_many_lines` allow GONE (`2d098f5`); dsl `needless_pass_by_value` ×2 re-audited — both allows GONE, `register_identifier_element` truly consumes its owned `RegItem` (clone saved), `add_perspective_to_holders` takes `&Perspective` (`55037ae`); rest re-audited = KEEP rows (arm-order `too_many_lines` ×7 “arm order IS the contract”, Java-contract casts — R0 allow audit stands). Wave end: full e2e tier green · bless 307×unchanged · doc-zero held · `bench cmp 4T` ALL 1.00 | `2d098f5` `55037ae` |
| **R2** | export/import — **DONE 2026-11-22**: typed `ImportFormat` + `ImportFormat::sniff` now live in `oxidizr-import` — the sniff table moved OUT of `oxidizr-api::pipeline` (seed CORRECTION: it was api-side, not cli-side; the cli `ImportCmdFormat` clap enum stays, the CLI grammar is frozen) and `oxidizr_api::ImportFormat` remains a stable re-export (`4cc628c`, FIRST churn-list entry); `regexlite`/`pathfilter` — **KEEP SPLIT DECIDED**, contract/semantics/scale rationale in §P4R-D5, no code. Wave end: full e2e tier green · bless 307×unchanged · doc-zero held · bench cmp SKIPPED with reason: import-command glue only, no bench code path touched (`e2e_*`/`m7_*`/`p3_*` never call the sniffer) | `4cc628c` |
| **R3** | api/inspect/layout — **DONE 2026-11-22**: `Request` per-domain audit complete — report domain typed: `json: bool` payloads → `format: DiffFormat`/`InspectFormat` api enums, CLI clap enums renamed `DiffCmdFormat`/`InspectCmdFormat` (`2520cd8`, churn 2); `ApiOptions` `struct_excessive_bools` allow GONE — `strict`/`keep_unknown` grouped into nested `parse: ParseFlags` (`0837317`, churn 3). KEEP rows: `Request::Export.format` STAYS `String` (`oxidizr_export::ExportFormat::parse` is the typed boundary — the reference class-loads arbitrary format names, the open-set contract needs no enum); inspect `<Type>Inspection` naming + export graph STAYS (const names are Java class names upper-cased = provenance, values are the `getType()` byte contract; surface = 3 re-exports + `types`, already minimal); remaining payload bools (`quiet`/`force`/`ignore_layout`) are honest one-flag values. Wave end: full e2e tier green · bless 307×unchanged · doc-zero held (new pub enums documented) · `bench cmp 4T` ALL 1.00 | `2520cd8` `0837317` |
| **R4** | lsp/cli/viewer/xtask/e2e: cli `report` renderer-plumbing review; viewer shell `struct_excessive_bools` split; `to_request` KEEP row documented (argv matrix IS the content) | R4 |
| **R5** | Public-API consolidation pass: `pub use` graphs, renames land, churn list → CHANGELOG lines, doc links green | R5 |

## P4R-D5 DECIDED (R2, 2026-11-22): `regexlite` and `pathfilter` stay SPLIT

The seed asked whether the two std-only backtracking engines share a core
worth extracting. Re-measured against the tree — **keep split**:

- **Different contracts.** `regexlite` (`oxidizr-import`) mirrors
  `java.util.regex` down to `find()` + group extraction and `{n}`
  `{n,}` `{n,m}` (the reference's own ADR/image patterns use `\d{4}`,
  `.+?`) because `!docs`/`!adrs` `exclude` is a reference
  `String.matches` leg. `pathfilter` (`oxidizr-api::generate`)
  deliberately does NOT build counted quantifiers or captures — those
  constructs are the OXZ-5010 USER diagnostic (a product exit-family
  contract), not a matcher gap. Unifying would either drag OXZ-5010 into
  `oxidizr-import` (crate boundary: import knows no CLI codes) or widen
  pathfilter's accepted grammar silently (an exit-family change — frozen).
- **Different semantics.** pathfilter NORMALIZES greediness (`*?` ≡ `*`
  — under a boolean full match greediness never flips existence);
  regexlite must PRESERVE leftmost-first preference (find results and
  group text depend on it). One engine matching both = more complex
  than either.
- **Scale.** The shared idea is CPS-continuation backtracking — a
  pattern, ~150 lines in `generate.rs` vs ~800 in `regexlite.rs`; a
  shared crate for one comment-level idea is churn, not dedupe.

## P4R-D4 Verification list

- P4R-V1 — bench baseline id after P4T — **CLOSED at R0: `4T`**
  (`cargo xtask bench save 4T`, 104 files committed; hot-path waves
  compare with `cargo xtask bench cmp 4T`).
- P4R-V2 — `cargo doc --no-deps` warning-free per crate — **CLOSED at R0:
  the pre-existing 43 warnings were FIXED (no allow-list needed); every
  wave re-checks `cargo doc --workspace --no-deps` at zero (the drift
  detector).**
- P4R-V3 — CHANGELOG skeleton — **DECIDED at R0: E-5 creates
  `CHANGELOG.md`; until then R waves accumulate breaking-ish lines in the
  staged running list `agents/notes/process/api-churn-v0.4.0.md`** (the
  skeleton landed at R0; the first churn commit adds its entry; E-5 folds
  the list into the changelog verbatim).

## Risks

- **API churn sprawl** (the 2R freeze existed to prevent exactly this):
  one API item per commit, wave scope capped by the R0 table; anything
  bigger becomes a PHASE_4R row or waits for the future mistake-fix phase.
- **Byte drift disguised as refactor**: the bless no-op + full tier per
  wave is the tripwire; a diff means stop and fix the cause.
- **Matrix rot**: test renames without matrix updates — R0 adds the
  matrix-row ↔ test-name consistency to the harvest script (P4T T-1).
- **No tag at close** (interlude precedent) — the churn ships in v0.4.0
  through the CHANGELOG lines, which is exactly why they are mandatory
  per commit.
