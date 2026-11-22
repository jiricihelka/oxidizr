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

## P4R-D2 Catalog seeds (found by the R0 inventory, not guessed)

- `Request` in `oxidizr-api/pipeline.rs` carries every command's argv
  shape — review variant payloads / group per-domain request enums.
- `generate.rs` merge engine: `HashMap<String, …>` ladders for the
  person/system merge; `pathfilter` public API shape.
- `inspect` severity strategy + ~35 modules: naming pass for the
  `<Type>Inspection` types vs `oxidizr_inspect::` export graph.
- importers: sniffer/dispatch stringly formats → `ImportFormat` coverage;
  `regexlite`/`pathfilter` share a regex-subset core? (dedupe candidate).
- cli: the `to_request` giant match + `command_name` map — derive or
  table; `report` renderer plumbing.
- 2R leftovers still carrying `#[allow(clippy::…)]` without a
  re-examination (audit every allow; keeps the ratchet honest).

## P4R-D3 Waves (each: quartet + full tier + bless no-op + commit)

| Wave | Scope | Commit |
|---|---|---|
| **R0** | Inventory: allow-audit, `cargo doc` baseline, bench baseline `4T`, wave table filled with real items | R0 |
| **R1** | `oxidizr-core` / `ast` / `diag` / `dsl` / `json` | R1 |
| **R2** | `oxidizr-export` / `oxidizr-import` (incl. regex-subset dedupe decision) | R2 |
| **R3** | `oxidizr-api` / `oxidizr-inspect` / `oxidizr-layout` | R3 |
| **R4** | `oxidizr-lsp` / `oxidizr-cli` / `oxidizr-viewer` / `xtask` / `e2e` | R4 |
| **R5** | Public-API consolidation pass: `pub use` graphs, renames land, CHANGELOG lines accumulate, doc links green | R5 |

## P4R-D4 Verification list

- P4R-V1 — bench baseline id after P4T (`bench cmp 4T`) — re-pin once at R0.
- P4R-V2 — does `cargo doc --no-deps` currently stay warning-free per
  crate (baseline before the churn — pre-existing broken links get fixed
  in R0 or allowlisted with a note).
- P4R-V3 — CHANGELOG skeleton (created in the release tail E-5): R-waves
  keep a staged `agents/notes/` running list if the file does not exist
  yet — decide at R0.

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
