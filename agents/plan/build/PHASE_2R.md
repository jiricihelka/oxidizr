# PHASE_2R.md — detailed plan: small-scope refactors of the ported code (interlude)

**Status: COMPLETE 2026-11-19** — waves PR0–PR6 all DONE (wave table §D5); exit gate
green incl. full `oracle verify`; `2R` bench baseline committed; no tag (1T/2T
precedent). Journal: PROGRESS.md §P2R.

**Interlude between Phase 2T (e2e export coverage) and Phase 3 (viewer + layout).**
The port so far (~40k lib LOC) was written to be *behaviorally* faithful — JSON/diagram
bytes are the contract — and the e2e + golden + oracle nets now guard that contract at
unprecedented breadth (313 corpus entries, export matrix, full-tier invariants, 12
corpus benches). 2R spends that safety net: remove **Java-isms in the Rust structure**
that even the workspace's enforced lint bar (`clippy all + pedantic`, `-D warnings`,
inherited via `[lints] workspace = true`) does not flag, while the bytes it produces
stay bit-identical. The
P1T/2T precedent stands: an interlude rides the net it didn't have before, and the net
pays for itself by catching latent problems on the way.

Locked premises (user decisions 2026-11-18, this file's §D0): **small scope only** —
no API-surface changes, no architectural reshapes (those wait for remaining
re-implementation work in P3+); **zero behavior change** — any golden / sidecar /
diagram-byte diff caused by 2R is a refactor bug, fixed at the cause, never blessed;
`testdata/` untouched; positioned **now, before P3**, while the port-era code is fresh
and P3 will build on the cleaned crates; **no tag at close** (1T/2T precedent — no
shipped-behavior change).

## P2R-D0 Charter and freeze scope (ratified 2026-11-18, user Q&A)

**In scope**: small refactors of Java patterns that are not idiomatic Rust —
accessor naming, needless `clone()`/`to_string()`, index loops vs iterators, `Option`
ladders, stringly-typed dispatch, out-params vs returns, `pub` over-exposure,
long-method decomposition with citation, string-building idiom (perf-flagged).

**Frozen (the contract)** — the only thing "no API changes" means:

| Frozen | Where enforced |
|---|---|
| `oxidizr-api` `Request`/`Response` surface + `execute` semantics | api crate tests + e2e suite |
| CLI argv grammar, exit classes, stdout/stderr usage | `oxidizr-cli/tests/cli.rs` |
| All byte outputs: workspace JSON goldens, diagram bytes, OXZ codes + exit classes | goldens (Cat A), e2e sidecars, oracle (Cat B) |
| `testdata/` and blessed e2e sidecars | a 2R wave that *changes* one of these is red by definition |

**Free**: pub-in-crate fn signatures, private items, intra-crate module layout,
internal naming — call sites follow in the same commit. (User decision: contract
freeze, not signature freeze.)

**Out of scope**: new lints-as-architecture (only §D5's ratchet), dependency changes,
error-taxonomy redesign, anything DEVIATIONS.md Locked, perf *optimizations* beyond
what a mechanical idiom fix happens to bring (claims still need criterion evidence,
WORKFLOW §Code quality).

## P2R-D1 Safety net: what must stay green, when

- **Every commit**: quartet (`cargo xtask ci`) — goldens + quick tier + refs included.
- **Every wave**: full tier (`OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e`, incl.
  `full_export` invariants) **plus a bless no-op**: `cargo xtask e2e bless` after the
  wave must produce **zero git diff** — with the 2T export matrix this is a 313-entry
  × 5-axis behavioral invariance proof, not a spot check.
- **Wave that touches a hot path** (lexer, fmt, writers, export engines): `cargo xtask
  bench cmp 2T` — a > ~10 % swing on any `e2e_` bench is reviewed before merging the
  wave (TESTING.md §Benchmarks rules; id renames excluded, they count as the same bench).
- **Once at close**: `cargo xtask oracle verify` (Category B, slow) — belt on the
  "zero behavior change" promise, then `cargo xtask bench save 2R` + commit baseline.

## P2R-D2 Audit trail: cite-and-decompose (ratified)

Decomposition of ported code is allowed **everywhere**, including byte-hot files, with
one rule: every extracted fn carries rustdoc citing the Java original it came from
(`/// Ported from `com.structurizr…#method`; 2R extraction`) or explicitly
(`/// Rust-side glue — no Java counterpart`). Witness/catalog docs (`WRITE-CATALOG.md`,
`EXPORT-*.md`) get their section/line citations updated **in the same commit** as the
code move; `xtask refs` (quartet step) is the integrity gate for doc references.
Net effect: the audit story moves from file-position 1:1 to citation 1:1 — strictly
more durable across refactors.

## P2R-D3 Findings registry: disposition classes

The 2026-11-18 survey (four crate-group sweeps: dsl / core / export / json+api+cli+
misc) populates the register (§Register below) with `R-<crate>-<nn>` items. Each gets
one disposition:

| Class | Meaning | Action |
|---|---|---|
| **SWEEP** | mechanical, zero judgment (naming, `explicit_iter_loop`, clone-demotion with obvious borrow) | fix in the crate's wave, one commit per pattern-family |
| **JUDGE** | behavior-preserving but per-site reasoning (param reshaping, `pub` shrink) | fix in the wave; commit message carries the judgment in ≤ 1 line per site group |
| **KEEP-CITE** | load-bearing faithful port (Java `hashCode` emulation, TreeSet-rebuild ordering, byte-shape string building) | leave; ensure a narrow `#[allow]` + faithful-port rationale exists if §D5's lints will flag it |
| **DEFER** | real but not 2R's scope (needs P3+ API/behavior thought) | stays in the register with a pointer, invisible to the waves |

## P2R-D4 Lint ratchet (corrected 2026-11-18 by the surveys: bar is `all + pedantic` already)

**Verified premise (CORRECTION to the draft):** the workspace root `Cargo.toml`
already sets `[workspace.lints.clippy] all = "warn" + pedantic = "warn"`, inherited by
every crate via `[lints] workspace = true` — so every candidate the draft listed
(`explicit_iter_loop`, `needless_pass_by_value`, `trivially_copy_pass_by_ref`,
`match_same_arms`, `unnecessary_wraps`) is **already enforced** under CI `-D warnings`.
The genuine lint blind spots are only: **nursery/restriction** families + non-lintable
habits. Hence the ratchet: after the sweep waves, try `redundant_clone` (nursery —
unstable, so it needs a `NIGHTLY.md` register row + measured compile-time hit) and
nothing else workspace-wide; `&x.clone()`-style waste and accessor naming stay
survey+review territory (no lint exists). Kept additions land as workspace-lint rows +
a WORKFLOW.md §Code style note.

**V-R2 decision table (measured 2026-11-18, post-PR5 tree, pinned nightly):**

| Probe | Result |
|---|---|
| Fired sites, `-W clippy::redundant_clone` over `--workspace --all-targets` | **11** — `stmt_deployment.rs:380,390,445` (`technology` after its match/leg tail: never used again), `engine/mod.rs:1746–1772` ×6 (`start_group_context`: one of six exclusive arms runs, each can MOVE the by-value `group` — the crate comment already says callers hand it over done), `oxidizr-api/tests/default_views.rs:179`, `oxidizr-e2e/tests/full.rs:229` (last-use clones in test bodies) |
| Site audit | **11/11 genuine last-use clones, 0 false positives, 0 allows needed** (kill switch ≤ 10 → far below). Fixes are `clone()` → move — the `&x.clone()`-waste family 2R sweeps by hand anyway. |
| Compile-time delta (cold `cargo clippy --workspace --all-targets`, deps warm, 51 units/rebuild verified, n=3 each) | baseline 2367/2161/2365 ms (med 2365) vs nursery 2612/2466/2398 ms (med 2466) → **≈ +100 ms (+4 %), ranges overlapping — noise-level** |
| Verdict | **KEEP**: `redundant_clone = "warn"` joins `[workspace.lints.clippy]` (CI `-D warnings` binds), `NIGHTLY.md` register row added (nursery = unstable; drop the row + the lint if it ever stabilizes), the 11 sites fixed in the same commit. |
Related hygiene the survey found (folded into PR1/PR3): `faithful-port.md`'s
narrowest-scope rule is violated by the module-level
`#![allow(missing_errors_doc, missing_panics_doc)]` in core `builder.rs:14`/
`viewops.rs:15` (resolved: kept as a recorded crate convention, documented at PR6).
The `unwrap_used` allow in core `url.rs:113` the survey flagged as a no-op outlier is
**KEPT** — corrected at PR1-d: 28 files carry the identical test-module allow as the
charter's `unwrap`-only-in-tests guard.

## P2R-D5 Waves (ordered, each ends green — quartet + full tier + bless no-op + commit)

Size gate honored: each wave is ≤ ~4 modules and gets its registry-item list in this
file before starting.

| MS | Deliverable | Exit gate |
|---|---|---|
| **PR0** | **DONE 2026-11-18** — register populated from all four crate sweeps (65 rows incl. perimeter), dispositions complete (SW 32 / JU 22 / KC 9 / DF 1 + SW/DF 1); survey premise corrections recorded above (lint bar `all+pedantic` — §D4 rewritten; ast `RefCell` claim FALSE; naive brace-tool long-fn measurements wrong — real targets listed). The surveys' verdict: crates are far more idiomatic than feared — real prey = the dsl dispatch-clone family (R-dsl-01/02, the crate's entire perf gap), core method-body habits (clones/`expect` panics/duplicated lookups), export write-loop clones + the cited decompositions, and the api `Outcome` plumbing shape | register + dispositions 100 %; quartet green (docs commit) |
| PR1 | `oxidizr-core` wave — **DONE 2026-11-18**, 4 commits: PR1-a `2359084` clone waste (R-core-01/02/04), PR1-b `cb9cbdd` shared lookup helpers + borrowed flatten + single-pass view keys (R-core-03/09/10/11), PR1-c `2b8d43a` `expect`-panic elimination (R-core-07), PR1-d `1b90568` parity annotation + pub shrink + snapshot-clone removal + rename + keep-markers (R-core-05/06/08/13/14/15). Every commit: quartet + full tier 8/8 + bless no-op. `bench cmp 2T` (pinned worktree at `1b90568`, uncontended): ALL groups faster-or-equal — `m7_dsl_parse_big-bank-plc` 3.0→1.22 ms, `m7_reconcile_cross_corpus` 441→182 µs, `m7_reconcile_matched_big_bank` 692→312 µs, `e2e_reconcile_166` 1146→665 µs, `m7_dsl_parse_corpus` 13.7→9.2 ms, `m7_json_write_corpus` 2.3→1.85 ms, export/e2e legs 1.1–1.5× | green + `bench cmp 2T` reviewed ✓ |
| PR2 | `oxidizr-dsl` wave — **DONE 2026-11-18**, 5 commits: PR2-a `c09bbcd` shared-borrow dispatch preamble — `Tokens` Cow-slice + `&Stmt` sections (R-dsl-01, +05 fell out), PR2-b `0d8026a` `Tokens::get/last/first` → `Cow<'_, str>` + `eq_at` + compiler-driven call-site sweep (R-dsl-02/04), PR2-c `6cc507f` let-chain/`write!`/counter/hoist (R-dsl-03/11/12/13), PR2-d `63cbc5a` pub surface → parse API + dead-code settle-out (R-dsl-09), PR2-e `fd3c257` fresh `p.top()` per arm — whole-`Ctx` prelude clones gone (R-dsl-06; deployment/model amend out). BYTE-PATH: every commit quartet + full tier 8/8 + bless no-op (corpus-diff=0). `bench cmp 2T` on main at `fd3c257` (cumulative PR1+PR2): ALL groups faster-or-equal — `m7_dsl_parse_big-bank-plc` 3.0→1.07 ms, `m7_dsl_parse_corpus` 13.7→7.2 ms, `m7_reconcile_cross` 441→186 µs, `e2e_reconcile_166` 1146→573 µs, `e2e_dsl2json_all` 14.6→10.2 ms, `e2e_validate_all` 11.0→6.4 ms, export legs 1.5–1.9× | green + `bench cmp 2T` reviewed ✓ |
| PR3 | `oxidizr-export` wave — **DONE 2026-11-18**, 8 commits: PR3-a `b1b295e` clone/alloc cluster (R-exp-01/02/03/04/06/07/13), PR3-b `a64da54` `IndentingWriter` fill-loop (R-exp-05, own bench record `041ff22`), PR3-c `7bfbac8` c4 cascade helper + one-shot dispatch + stale attrs (R-exp-14/17/18), PR3-d `8a1e763` memoize `relationship`/`canonical_name` + pub shrink (R-exp-08/09), PR3-e `70ccde4` themes chain + clone-free load (R-exp-16), PR3-f `daf8e24` `render()` per-style split (R-exp-10), PR3-g `307518b` `body`/`export_dynamic_sequence` splits (R-exp-11), close = KC ratifications (R-exp-12/15, container-leg cite added). Every commit: quartet + full tier 8/8 + bless no-op (corpus-diff=0). `bench cmp 2T` at close: ALL 23 groups faster-or-equal — export cluster 1.37–1.92× (`export_plantuml_all` 17.9→13.0 ms, `plantuml_biggest_166` 616→354 µs, `c4_biggest_166` 487→256 µs, `mermaid_biggest_166` 495→258 µs), merged legs `dsl2json_all` 14.6→10.2 ms, `validate_all` 11.0→6.6 ms, `reconcile_166` 1146→597 µs; the R-exp-08 memoize increment over PR3-b numbers is within noise (biggest_166 export legs already 335/239 µs — correctness of the memo shape is the claim, criterion evidence = these tables) | green + `bench cmp 2T` reviewed ✓ |
| PR4 | `oxidizr-json` + `oxidizr-api` + `oxidizr-diag` + `oxidizr-ast` + `oxidizr-cli` wave — **DONE 2026-11-18**, 6 commits: PR4-a `81eb675` json writer cluster — infallible `set_list`/`list` + `try_set_list` with the `decisions` swallow kept as byte contract (R-mis-09), borrowed `InstanceTail` + allow gone (R-mis-02), `RvSortKey` precomputed sort keys, `dynamic: bool` gone (R-mis-03), `ThemeView` borrowed serialization (R-mis-08), allow-gap audit incl. `golden_m3.rs` rationale verify (R-mis-13), PR4-b1 `139581d` printer dead-identical array arms collapsed + Jackson comment (R-mis-06), PR4-b2 `a371afc` reader rels return-and-flatten, push order preserved exactly (R-mis-07), PR4-c1 `1db1b5c` Result-shape bodies: private `Fail(Outcome)` flattened at the frozen `execute` boundary + `statement_errors` helper (R-mis-01/05), PR4-c2 `ea9ad45` `decorate` `&&` let-chain + `to_request`/`replay` move semantics (R-mis-04/10), PR4-c3 `8a67f7a` reconcile named id-space builders + first-wins set lookups (R-mis-11); R-mis-12 audit-trail ratified at close — **all 13 R-mis rows dispositioned** (the two crates without rows: diag and ast — survey found none). BYTE-PATH wave: every commit quartet + full tier 8/8 + bless no-op (corpus-diff=0). `bench cmp 2T` at close: ALL 23 groups faster-or-equal — the json byte-path legs `json_read_corpus` 1234→1087 µs / `json_write_corpus` 2.3→1.96 ms, merged `dsl2json_all` 14.6→9.7 ms, `validate_all` 11.0→6.5 ms, `reconcile_166` 1146→567 µs, `plantuml_biggest_166` 616→348 µs, `c4_biggest_166` 487→255 µs, `m7_reconcile_cross` 441→192 µs | green (byte-path: json writer per D2 cite rule) + `bench cmp 2T` reviewed ✓ |
| PR5 | `xtask` + `oxidizr-e2e` harness sweep (least constrained) — **DONE 2026-11-18**, 2 commits + close: PR5-a `371a593` xtask — `run_oracle_export` shared prologue (json/plantuml/theme trio), verify legs `failures: &mut usize` → `Vec<String>` folded in `cmd_oracle_verify`, the two non-test `unwrap()`s → `with_context` (R-tool-01/02), PR5-b `42bff5b` e2e — explicit json2json feed (double-default swallower), `codes_from_value` ×2 dedupe, mutate op prologue `pick_index`/`pick_line` with the seeded draw sequence preserved bit-for-bit (R-tool-04); close = R-tool-03 ratify (keep, no code). Every commit: quartet + full tier 8/8 + bless no-op (corpus-diff=0 — the blessed mutated-corpus dir names prove the rng-draw invariance). `bench cmp 2T` at close: ALL 23 groups faster-or-equal — engine legs match the PR4-close numbers within noise (PR5 touches no engine crate: `m7_dsl_parse_big-bank-plc` 3.0→1.05 ms, `m7_reconcile_cross` 441→177 µs, `m7_json_read_corpus` 1234→1046 µs, `plantuml_c4_biggest_166` 487→245 µs) | green + `bench cmp 2T` reviewed ✓ |
| **PR6** | **DONE 2026-11-19** — `d890ef9` V-R2 KEEP landed: `redundant_clone = "warn"` in `[workspace.lints.clippy]` + `NIGHTLY.md` register row + all 11 fired sites fixed (`clone()`→move: `start_group_context` ×6 arms + its obsolete `needless_pass_by_value` allow, `technology` legs ×3, test last-uses ×2) + §D4 decision table; `6baa7b4` `bench save 2R` (23 groups) + README context rows; close = docs (WORKFLOW §Code style ratchet row + R-core-13 conventions row, notes: pitfalls/shell-timing-and-cargo-clean + process/refactor-patterns, PROGRESS board + log, RESUME) | quartet + full tier 8/8 + bless corpus-diff=0 + `oracle verify` **4 json + 90 diagram + 6 theme match** (= first real exercise of PR5's refactored verify legs) + `bench cmp 2T` ALL 23 faster-or-equal ✓; **P2R COMPLETE — no tag** |

## P2R-D6 Verification list (empirical, before dependent code)

- **V-R1** (before PR1) — invariance harness sanity: run the wave ritual (full tier +
  bless) on an **unmodified** tree and record the no-op — proves the ritual itself is
  meaningful before the first behavior-adjacent edit lands. **CLOSED 2026-11-18**:
  full tier 8/8 ok + `cargo xtask e2e bless` → every entry `unchanged`, corpus git
  diff = 0 lines, at HEAD `7e1fe73` (pre-2R). The ritual is a real net.
- **V-R2** (before PR6) — ratchet measurement: try `redundant_clone` (nursery) on the
  post-sweep tree; record (a) fired-site count, (b) compile-time delta, (c) false-
  positive notes → keep/drop decision table under §D4. Decision table recorded even
  when the verdict is drop. **CLOSED 2026-11-18**: 11 fired sites, all genuine; ≈ +4 %
  cold-clippy cost (noise); verdict **KEEP** — table under §D4.
- **V-R3** (per wave) — bench review: `bench cmp 2T` numbers posted in the wave's
  commit message (one line), regressions > ~10 % block until explained or reverted.

## Risks

- **Behavior drift** — the whole game. Mitigated structurally: D1's bless no-op is a
  whole-net behavior diff, not a spot test; drift is caught the day it is introduced,
  not at close.
- **Audit-trail erosion** — D2's citation rule + `xtask refs` gate; catalog docs move
  with the code in the same commit.
- **Ratchet noise** — the ≤ 10-allow threshold is an empirical kill switch (D4); any
  lint that fights the port *is* the wrong lint for this codebase.
- **Effort vs P3 overlap** — P3 is mostly *new* crates (viewer, layout engine); it
  consumes core/api/export as libraries, so cleaner internals pay forward instead of
  evaporating. Register DEFER items are the pressure valve.
- **Mechanical churn hiding real changes** — waves commit by pattern-family (D3), so
  any suspicious diff is isolated to its family's commit; review per family, not per wave.

## Survey corrections (verified 2026-11-18 — trust these over the draft premises)

- **Lint bar**: workspace lints are `clippy all + pedantic = "warn"`, `-D warnings` in
  CI (root `Cargo.toml [workspace.lints]`, inherited via `[lints] workspace = true`).
  §D4 rewritten accordingly.
- **`oxidizr-ast` has NO `RefCell`/`Cell`** anywhere — the owned AST uses `Rc<str>`
  for the source; the parser is already ownership-based. Nothing to convert.
- **Long-function measurements from naive brace tools were wrong**: `viewops.rs:1200`
  is a 32-line fn (top-5 real: `check_element_can_be_added` 162, `create_default_views`
  125, `view_add_default_elements` 124 — all cited); `engine.rs:1208` is a 14-line fn.
  Real decomposition targets: `structurizr.rs render()` (174 ln) and, as name-pinned
  decompositions, `body` (107 ln) / `export_dynamic_sequence` (125 ln). The dsl
  `section()` chains (553/517/364/320 ln) are the *documented contract* — keep.

## Register (PR0 — populated from the four crate sweeps, 2026-11-18)

Dispositions per §D3: **SW** = sweep, **JU** = judge (per-site reasoning, wave
commit), **KC** = keep-cite (ratified Java shape), **DF** = defer (out of 2R).
Waves: 1 core · 2 dsl · 3 export · 4 json/api/cli · 5 tooling · 6 ratchet/close.

### oxidizr-dsl

| ID | finding (file:line) | dis | wv |
|---|---|---|---|
| R-dsl-01 | dispatch preamble re-clones tokens/ctx 7× per line (`engine/mod.rs:69`, 7 × `section` heads) — share borrows (`section(p, &stmt)`); no section mutates `stmt` | SW | 2 |
| R-dsl-02 | `Tokens::get` allocates 2 Strings per read (`tokens.rs:22`) → `Cow<str>` (skip replace when no `\\`), `eq_at` for compare-sites; `last()` re-entry fix; keep double-pass ORDER (erratum 26) — **DONE PR2-b** (Cow get/last/first + `eq_at` at the two `==` compare-sites; validate_view_key takes Cow, owns on success) | JU | 2 |
| R-dsl-03 | `if let (true, pat) = (cond, val)` tuple hack (`find.rs:43`) → let-chain — **DONE PR2-c** | SW | 2 |
| R-dsl-04 | `Option<&String>` params ×4 (`stmt_views.rs:552,600,710,882`) → `Option<&str>` — **DONE PR2-b** (folded into the Cow conversion: `Some(&token)` sites) | SW | 2 |
| R-dsl-05 | `java_trim(…).to_string()` per line (`stmt_line.rs:30`) — borrow is enough — **DONE PR2-a** (the per-line copy fell out of the shared-borrow dispatch preamble `c09bbcd`; `java_trim` returns `&str` and the `:30` site reads a borrow) | SW | 2 |
| R-dsl-06 | `p.top().cloned()` heavyweight Ctx clones (`stmt_props.rs:99,282`, `stmt_deployment.rs:100`, `stmt_model.rs:361`, `stmt_style.rs:26`, `stmt_views.rs:1562`) — extract only the payload Copy fields, re-borrow — **DONE PR2-e** (guard ladders + payload arms converted in props/style/views; `stmt_deployment.rs:99` and `stmt_model.rs:359` amended out: their `.cloned()` already extracts one payload into locals — no whole-Ctx cost pattern the row targets) | JU | 2 |
| R-dsl-07 | `section()` chain fns 553/517/364/320 ln + `permitted_tokens` 300 ln — arm order IS the contract; allows carry rationale | KC | — |
| R-dsl-08 | stringly archetype `kind: String` ladder (`ctx.rs:60`, `stmt_style.rs:85-108`, `mod.rs:1790-1798`) — enum would surface the `_ =>` InfrastructureNode fallback but rewrites the hot parse path; LSP (P4) is the natural owner | DF | — |
| R-dsl-09 | engine `pub use` sprawl (`engine/mod.rs:50-55`, `KindFilter`) — externals consume only parse surface → `pub(crate)` demotions — **DONE PR2-d** (`DslError` kept `pub` for the api; surfaced dead code settled per R-core-06 policy: `Tokens::sublist` deleted, Ctx/idents parity helpers + D7 holder payloads kept + annotated) | JU | 2 |
| R-dsl-10 | Java getter names (`Parser::get_*`, whole `Tokens` API) — rustdoc cites `Tokens.java`/`context.getElement`; ratify keep | KC | — |
| R-dsl-11 | `push_str(&line.to_string())` (`error.rs:109-110`) → `write!` (text identical, golden-guarded) — **DONE PR2-c** | SW | 2 |
| R-dsl-12 | Java `count` counter lockstep (`rel.rs:457-475`) → `removed.is_empty()`; `stmt_views.rs:1290` clone shrinks with R-dsl-02 — **DONE PR2-c** (`:1290` clone already gone: PR2-a borrow slice + PR2-b Cow) | SW | 2 |
| R-dsl-13 | `.to_lowercase().ends_with(".json")` computed twice (`mod.rs:1452,1499`) — hoist once; do NOT `Path::extension`-ify (reference shape) — **DONE PR2-c** (`is_json`) | SW | 2 |
| R-dsl-14 | bool-flag params (`apply_text title`, `dynamic_step open`, `relationship_endpoint source`…) — all in byte-contract chains; ratify skip | KC | — |
| R-dsl-15 | 31 `#[allow]` sites: all narrow + rationaled — model for other crates | KC | — |

### oxidizr-core

| ID | finding (file:line) | dis | wv |
|---|---|---|---|
| R-core-01 | waste clones: `&id.clone()` (`builder.rs:1452`), loop clones ×3 (`:791,800-805`) — **DONE PR1-a** `2359084` | SW | 1 |
| R-core-02 | 7 element constructors clone the whole item to return its id (`:254,272,301,338,376,411,483`) — clone `Id` (Arc bump) once, move item — **DONE PR1-a** `2359084` | SW | 1 |
| R-core-03 | `has_element`/`element_kind` re-run cloning `flattened()` per call (`:2259-2272`) — borrowed twin `flattened_ids` (`validate.rs:328`) → `pub(crate)`, reuse (DRY + gate-path allocation storm) — **DONE PR1-b** `cb9cbdd` | SW | 1 |
| R-core-04 | `.to_string()` inside `.filter()` per rel-view (`viewops.rs:1223-24`) — membership on `Vec<&Id>` — **DONE PR1-a** `2359084` | SW | 1 |
| R-core-05 | snapshot-clone habits (`viewops.rs:835-841` endpoints, `:1305-1317` RelInfo) — **DONE PR1-d** `1b90568` (collect-then-add; `RelInfo` deleted, borrows `&[Relationship]`) | JU | 1 |
| R-core-06 | pub shrink: dead parity API (`element_tag_defaults:1339`, `relationship_technology:1212`, `add_tags:1920`, `efferent_relationships:1053`, `implied_strategy:103`, `set_workspace_id:168`) → keep + `#[allow(dead_code)]` + "Java parity surface" comment; internal-only (`has_element`, viewops trio, `item_mut`, ref/mut getters) → `pub(crate)`; `found:126` → private — **DONE PR1-d** `1b90568` (all verified workspace-wide; `found` kept `pub` + allow: it is the `IdGeneratorStrategy` parity surface, used by ids tests) | JU | 1 |
| R-core-07 | `expect("checked")` library panics (CHARTER violation): `viewops.rs:430,451` + check-then-mutate `.expect("checked above")` ×~10 (`builder.rs:335,373,443,480,508,554,780,859,891,1447`, `viewops.rs expect_content:1563`) — single-lookup Result with identical messages — **DONE PR1-c** `2b8d43a` (`expect_content` → `model_content_mut` + additive `CoreError::NoViewContent`) | JU | 1 |
| R-core-08 | out-param closure with stop-signal (`element_technology`, `builder.rs:1169-1208`) — **DONE PR1-d** `1b90568` (returns through the walk's own `Option<T>`) | JU | 1 |
| R-core-09 | 16× `find(\|r\| &r.id == id)` — add borrowed `relationship()` twin of `relationship_mut`, rewrite sites — **DONE PR1-b** `cb9cbdd` | SW | 1 |
| R-core-10 | instance lookups rebuild full `flatten_nodes` (clones group Vecs) per gate call (`viewops.rs:503-544`) — recursive `find_env`/`find_instance`, no group data needed — **DONE PR1-b** `cb9cbdd` | SW | 1 |
| R-core-11 | `new_view_base`/`generate_view_key` double `all_views()` O(n²) (`builder.rs:2161-2208`) — one collected pass; `ViewRef::slot` helper kills 9 copy-paste tails — **DONE PR1-b** `cb9cbdd` | SW | 1 |
| R-core-12 | **DO-NOT-TOUCH order perimeter** (0 violations found): `partition_point`+`style_cmp` (`views.rs:641-679`), `id_sort_key`, `java_cmp_str`, custom-elements reverse-insert, keep-first-users, hydrate-order sort, writer-sorted notes — WRITE-CATALOG §5 | KC | — |
| R-core-13 | allow hygiene: `#![allow(unwrap_used)]` in `url.rs:113` **KEPT** (implemented 2026-11-18: it matches the workspace-wide test-module convention — 28 files carry the identical allow as the charter's `unwrap`-only-in-tests guard; the survey's "no-op outlier" read was wrong). Module-scope `missing_errors_doc/missing_panics_doc` allows (`builder.rs:14`, `viewops.rs:15`) KEPT as a recorded crate convention — note both in WORKFLOW §Code style at PR6 | KC | — |
| R-core-14 | `Tags::set_from_csv` is a constructor in setter clothing (`tags.rs:117`, one json-reader caller) → honest name — **DONE PR1-d** `1b90568` (→ `from_json_csv`) | JU | 1 |
| R-core-15 | keep markers + doc promotes: `styles.rs` copyFrom ×26 "do not fold" marker; `check_element_can_be_added`/`animation_step_relationships` rustdoc cites; `let _ = name;` destructure; validate HashMap-vs-quadratic style unify → DF (byte-adjacent, low value) — **DONE PR1-d** `1b90568` (markers + rustdoc promotes; the validate unify taken as DF) | JU | 1 |

### oxidizr-export

| ID | finding (file:line) | dis | wv |
|---|---|---|---|
| R-exp-01 | `is_visible` clones `Frame` per element×frame (`engine.rs:1159`) → `match &self.frame` — **DONE PR3-a** (borrows `self.frame.as_ref()`) | SW | 3 |
| R-exp-02 | `rels.clone()` for a trailing `is_empty()` (`engine.rs:900`) — hoist flag, iterate by ref — **DONE PR3-a** | SW | 3 |
| R-exp-03 | index-`remove` dedup O(n²) (`engine.rs:1497-1506`) → `Vec::dedup_by` (same keep-first-of-run; run `bench cmp 2T`) — **DONE PR3-a** (covered by the wave-close `bench cmp 2T`) | SW | 3 |
| R-exp-04 | `order.clone()` per comparator call (`engine.rs:1487-1492`) — inline into `format!` with `as_deref()`; key shape stays a `String` (TreeSet key = contract) — **DONE PR3-a** | SW | 3 |
| R-exp-05 | `IndentingWriter` per-line: `padding()` String + `replace()` even w/o `\n` (`indent.rs:68-90`) — fill loop + `contains` guard; own commit + bench (bytes guarded by whole export net) — **DONE PR3-b** `a64da54`; per-item bench at the commit: export legs vs `2T` all faster-or-equal (`export_plantuml_all` 17.9→13.3 ms, `plantuml_biggest_166` 616→335 µs, `c4_biggest_166` 487→239 µs, `mermaid_all` 13.7→9.0 ms); increment over the PR2-close numbers is within noise — the two removed allocations are small next to the `format!` cost per line, kept as the idiomatic shape | JU | 3 |
| R-exp-06 | `color_or_null(Option<&String>) -> String` (`structurizr.rs:1253`) → `Option<&str> -> &str`, 9 callers `.as_deref()` — **DONE PR3-a** (22 caller sites, not 9: structurizr 11, c4 5, mermaid 6) | SW | 3 |
| R-exp-07 | `to_string().into_bytes()` double copy (`naming.rs:75,84,95`) → `as_bytes().to_vec()` — **DONE PR3-a** | SW | 3 |
| R-exp-08 | `Engine::relationship()` linear scan per rv AND per is_visible leg; sequence sibling re-probes `canonical_name`; mermaid `sequence_document` `view_relationships()` ×2 — memoize (criterion-verifiable vs 2T) — **DONE PR3-d** (`Engine.rel_index` lazy `OnceCell<HashMap>` first-id-wins = old `find` semantics; `name_index` `RefCell<HashMap>` memo on `canonical_name` — id-pure over `'a`, recursion re-enters the wrapper; mermaid double probe hoisted to one `view_relationships()`. Side effect: `HashMap<&'a str, _>` makes `Engine<'a>` invariant — three elided mermaid helpers got a named `'a`; `Engine::new` loses `const` (`HashMap::new` isn't const). Criterion evidence: wave-close `bench cmp 2T`) | JU | 3 |
| R-exp-09 | pub shrink: only `ExportFormat`/`ExportError`/`ThemeStore`/`ThemeError`/`ExportedFile` cross the crate; `Engine::workspace()/frame()` have zero callers → `pub(crate)` (publish=true — shrink deliberately) — **DONE PR3-d** (both demoted + `#[allow(dead_code)]` Java-parity comment, R-core-06 shape; zero callers verified crate-wide and externally — consumers use only the five types listed) | JU | 3 |
| R-exp-10 | `render()` 174 ln → per-style `render_*` fns, each citing its `PlantUML*Style#toString`; allow gone; catalog docs don't cite `render()` — doc-safe — **DONE PR3-f** (`render` is now the dispatcher + `render_root`/`render_legend` (Self-less: no fields, avoids pedantic `unused_self`)/`render_element`/`render_relationship`/`render_deployment_node`/`render_boundary_or_group` (the two Java classes share the field set — one fn, both cites, like the old or-pattern arm); `too_many_lines` allow gone) | JU | 3 |
| R-exp-11 | `body` (107 ln) split per view-kind leg citing Java overloads; `export_dynamic_sequence` inner blocks split (sibling-insert + boundary-stack = "Rust glue"); **keep fn names** — cited at `EXPORT-MERMAID.md:141,146`, `PHASE_2.md:321-322`; catalog refs same-commit — **DONE PR3-g** (`body` = dispatch over `body_flat`/`body_container`/`body_component`/`body_dynamic`/`body_deployment`, each citing its reference `export(view)` overload — both `too_many_lines` allows gone; `export_dynamic_sequence` keeps its catalog-cited name/fn, its `elementsInWriteOrder` walk → `sequence_write_order`, boundary stack → `write_sequence_boundaries`) | JU | 3 |
| R-exp-12 | `type_of(_, include_metadata_symbols: bool)` — mirrors Java overload, doc-cited; prod calls all-`true` — **RATIFIED PR3 close** (keep: the `bool` IS the Java `typeOf(element, includeMetadataSymbols)` overload; doc cites `AbstractExporter.typeOf`; verified all 10 prod call sites pass `true`, only tests pass `false` — the parameter is the parity surface, no code) | KC | — |
| R-exp-13 | `canonical_name` reverse+push join (`engine.rs:1367-82`) → `chain.join("/")` w/ trailing-slash care; golden-verify — **DONE PR3-a** (slash keyed on `chain.is_empty()`; bless no-op = the golden verify) | SW | 3 |
| R-exp-14 | c4 color cascades ×4 near-identical (`c4.rs:582-603`) → `cascade_color` (two calls = the two Java cascades); drop `let element_to_write = element;`; `_view` param — **DONE PR3-c** (helper + alias gone + `_view`) | SW | 3 |
| R-exp-15 | `unwrap_or_default` on "unreachable" abort paths (`c4.rs:642,677`, `mermaid.rs:128`) — behavior is the contract; keep + cite — **RATIFIED PR3 close** (keep all three; cites verified at the c4 system leg + mermaid boundary, the missing cite on the c4 CONTAINER leg added at the close commit) | KC | — |
| R-exp-16 | themes: 2-var env loop stringly branch; `load_themes` clone iteration; silent-skips need EXPORT-PROPERTIES §6 cites where missing — **DONE PR3-e** (`install` chain written out in source order with an `env_dir` closure — no stringly `var ==` splice; `load_themes` iterates `configuration.themes` directly — `add_styles_from_theme` appends to the SIBLING `styles.themes` field so the borrows are disjoint, error timing unchanged; §6.1-3 cite on the `load_one` silent skip) | JU | 3 |
| R-exp-17 | stale/dead attrs: `paths.rs:49` `items_after_statements` allow (verify at implementation — it's a *style* lint under `all`, so re-check before deleting); `b64` inconsistent `try_from` vs `as usize`; unimplemented `WorkspaceExporter`/`DiagramExporter` traits = pub API → DF — **DONE PR3-c** (allow verified stale — clippy clean without it, deleted; b64 unified to mask-then-index; the trait stubs stay DF: pub API, no code) | SW/DF | 3 |
| R-exp-18 | below-cut cluster: add_style clone-then-discard, `engine.rs:348` collect-then-loop, `format.rs:143` double-dispatch collapse, `selector()` b64 recompute — **DONE PR3-c** (export() one match; add_style hoists tag/selector out of the dup closure — the by-value construct-then-drop on a dup mirrors the reference `HashSet.add`, kept; steps loop iterates directly) | SW | 3 |

### oxidizr-json / api / cli (+ diag, ast, e2e src)

| ID | finding (file:line) | dis | wv |
|---|---|---|---|
| R-mis-01 | `Outcome`-as-accumulator + 15× repeated `match x { Err(e) => { out.fail(…); return out; } }` in pipeline command bodies — internal helper Result, convert at the frozen `execute` boundary; partial-log semantics carried in the error — **DONE PR4-c** (private `Fail(Outcome)` abort arm: `out.fail(…)` now RETURNS the partial outcome — log so far + failing issue + exit class — every rung became `map_err(…)?`; `execute` flattens `command(…)?` at the frozen boundary; `load_export_workspace` returns `(Source, Workspace, recovered-issues)` instead of `Option` + `&mut Outcome` mutation — recovered D1 errors ride as data; `validate` keeps its Outcome (no fail legs — pure per-file aggregation) | JU | 4 |
| R-mis-02 | `deployment_instance_tail` 6 args / out-vec / `Option<&String>` (`writer.rs:715-732`) + unrationaled `too_many_arguments` allow — **DONE PR4-a** (borrowed `InstanceTail` param struct replaces the six args; the five keys RETURN for the callers' `insert_sorted` fold — no out-vec, `Option<&str>` via `as_deref`/`Json::opt_text`, allow gone) | SW | 4 |
| R-mis-03 | `relationship_views_with` comparator allocates 2 `format!` + re-parses f64 per comparison (`writer.rs:1368-95`); `dynamic: bool` params — precomputed sort keys; preserve stable-sort ORDER (`7/1, 9/2, 10/3, 11/3` witness) — **DONE PR4-a** (`RvSortKey` precomputes id/order text + f64 ONCE per item; `dynamic: bool` gone — `relationship_views`/`dynamic_relationship_views` ARE the two legs; `sort_by` stable so order identical incl. the witness; numeric-branch gate = `order_num.is_some()` = old parse-`is_ok` gate) | JU | 4 |
| R-mis-04 | tuple bool smuggling (`suggest.rs:120-126`) → plain `&&` (keep the guard the tuple encodes) — **DONE PR4-c** (the `if let (_, Some(list)) = (contains(…), split_once(…))` tuple became an `&&` let-chain — the tuple smuggled exactly `contains AND extractable-list`; both legs pure string ops so the short-circuit is unobservable) | SW | 4 |
| R-mis-05 | parse-error decoration ladder duplicated ×4 (`pipeline.rs:442,557,663,751`) → one helper — **DONE PR4-c** (`statement_errors(&ParseResult) -> Vec<Issue>`; the four `errors.iter().map(decorate(from_dsl(e), Some(ws)))` ladders — dsl2json/reconcile/validate/export-loader — all call it) | SW | 4 |
| R-mis-06 | dead-identical scalar-vs-object array arms in printer (`printer.rs:123-152`, verified) — collapse under goldens + Jackson-semantics comment — **DONE PR4-b** (one loop kept: `[ `/`, `/` ]` envelope + inline `emit` at `depth`; object arrays only LOOK expanded because the element `Obj` renders multi-line itself — stated in the collapsed comment) | JU | 4 |
| R-mis-07 | reader rels out-param threading (`reader.rs:266-294,395-465`) — return-and-flatten; order-independence per :546-549 comment (verify + goldens) — **DONE PR4-b** (`read_relationships` returns `(Vec<Id>, Vec<Relationship>)`; `read_node`/`read_infra_node`/`read_ssi`/`read_ci` return their collected rels beside the item, `read_model` is the single collector again; systems leg extracted to `read_software_system`; push order preserved EXACTLY (children→infra→ssi→ci→own per node, components→container→system per system, custom→nodes→people→systems per model) so even stable-sort ties on duplicate ids cannot move; goldens + full tier confirm) | JU | 4 |
| R-mis-08 | `theme_to_vec` deep clones for a single serialize (`theme.rs:62-71`) — borrowed entries — **DONE PR4-a** (`ThemeView` borrowed view; `theme_to_vec` serializes the LIVE workspace fields, `theme_write` borrows a bean via `ThemeView::of`; bean clones zero) | JU | 4 |
| R-mis-09 | `set_list(…).unwrap_or(None)` conflation ×~10 (`writer.rs:427,595,793…`) — infallible variant, drop the swallow-hazard — **DONE PR4-a** (infallible `set_list`/`list` are now the default, fallible legs moved to `try_set_list`; all 13 infallible swallows gone at the type level; the `decision` leg KEEPS the swallow as byte contract (out-of-range epoch → whole `decisions` key omitted — propagating would change pathological-input bytes, no golden coverage) with the hazard comment; clippy `unnecessary_wraps` proved the `software_system`/`container` Results phantom — wrappers dropped, `model()` cascaded infallible; only the 9 view legs + `decision` stay fallible) | JU | 4 |
| R-mis-10 | cli `to_request` clones a dead `Cli` (`lib.rs:184-235` → move) + replay issue clones (`:270-283`) — **DONE PR4-c** (`to_request(command: Command)` MOVES — every `output.clone()`/`files.clone()` leg gone, `run` moves `cli.command` in after the `ApiOptions` fields; `replay(renderer, outcome: Outcome)` drains the log: issues step into the render batch instead of cloning per item) | SW | 4 |
| R-mis-11 | reconcile `rels: bool` ×2 + closure shadows `fn map_ids` + `contains`/`find` scans (`reconcile.rs:53,143,346,364,423`) — **DONE PR4-c** (`Index::build`/`map_ids` bool switches split into named `build_relationships`/`build_elements` + `old_relationship_ids`/`old_element_ids` collectors feeding `map_ids(olds, new_idx, what)`; the `remap_animations` closure renamed `remap_ids` (no more shadowing the module fn); `graft_set`'s `new_keys` became a `HashSet`, `graft_content`'s per-entry `find`/`any` scans became id-keyed lookups — `entry().or_insert()` keeps the FIRST entry per id, the exact answer the old `find` gave (duplicates are a validation violation but reconcile runs BEFORE validation), and the new-id sets are built after the graft loops' mutable borings) | SW | 4 |
| R-mis-12 | javahash O(n²) emulator + reader `check_instances`/`two_star_shape` bug-compat — audit-trail — **DONE PR4 (audit, no code)**: ratify keep. `javahash.rs` IS the byte contract replay — its module doc carries the full semantics (spread/capacity/bucket order, `m3-order.json` witness, treeified-bucket deviation = `DEVIATIONS.md` D8; the bucket×keys scan is the emulated `HashMap` iteration, not an accidental quadratic); `check_instances`/`two_star_shape` carry the verbatim-reproduction comments (unescaped regex dots, backtracking `"12N"` witness). Cites verified present 2026-11-18 | KC | — |
| R-mis-13 | allow gaps: `writer.rs:715` (with R-mis-02) + `golden_m3.rs:81` test allow rationale — **DONE PR4-a** (`writer.rs` allow deleted with the R-mis-02 rework; `golden_m3.rs` allow verified to carry its 4-line rationale comment — audited, no change needed) | SW | 4 |

### xtask / oxidizr-e2e harness

| ID | finding (file:line) | dis | wv |
|---|---|---|---|
| R-tool-01 | 3× oracle runner triplication + `failures: &mut usize` out-params (`xtask main.rs:685-784,1186-1310`) → `run_oracle_export` + `Vec<String>` results — **DONE PR5** (`run_oracle_export(oracle, workspace, outdir, format)` = the create-dirs + run + exit-status prologue all three runners shared — the json leg's failure message now names `json` like the others did; `verify_json_legs`/`verify_diagram_legs`/`verify_theme_legs` answer `(counts…, Vec<String>)` failure lines, `cmd_oracle_verify` folds + prints them after the streaming `ok:` lines and bails with the count) | SW | 5 |
| R-tool-02 | `unwrap()` in non-test xtask (`main.rs:418,487`) — charter violation → `with_context`/`?` — **DONE PR5** (bench save/restore `p.file_name().unwrap()` → `with_context(…)?`; remaining `.unwrap()`s verified all inside `mod tests`; wave-close scan found one more same-class panic — `cargo_cmd()`'s `expect("repo root")` — → `cargo_cmd() -> Result<Command>`, five call sites `?`-propagate; xtask now has ZERO non-test `unwrap`/`expect`) | SW | 5 |
| R-tool-03 | oracle_gen bool-flag ladder (`main.rs:1059-1096`) — validated mutually-exclusive; borderline, ratify keep — **DONE PR5 (ratify, no code)**: the four bool flags parse a flat argv scan and are validated mutually exclusive by the `ensure!` right below the loop (`--c4` implies `--plantuml`; `--plantuml`/`--mermaid`/`--theme` ≤ 1); a flag-enum rewrite would re-shape the `GenMode` dispatch for zero behavior gain — keep | KC | — |
| R-tool-04 | e2e runner double-default swallower (`runner.rs:63`); manifest codes-parse ×2 (`manifest.rs:400,462`); mutate 5× prologue (`mutate.rs:67-153`) — dedupe prologue, keep seeded index selection — **DONE PR5** (`runner`: the guard proves `dsl_out` is the successful dsl2json, feed = `and_then(bytes)` + one explicit empty fallback; `manifest`: `codes_from_value(obj, at)` shared by both parsers, error strings unchanged; `mutate`: `pick_index` (empty = no draw, else exactly one `rng.below` — the old inline draw sequence) + `pick_line` prologue for the four line-scan ops, `op_dup_name` keeps its depth scan and joins the shared pick; full tier + bless corpus-diff=0 prove the blessed `m<i>-<label>-…` dirs byte-identical) | SW | 5 |

### Perimeter (all waves respect)

DO-NOT-TOUCH without a WRITE-CATALOG re-check: dsl Vec-as-LinkedHashSet stand-ins
(D8 comments), core order-perimeter (R-core-12), json javahash/reader leniency/printer
ordering, export comparator key shapes + HashSet-order index loops
(`structurizr.rs:790-847`, `c4.rs:228-290`).
