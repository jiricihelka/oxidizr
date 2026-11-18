# PHASE_2R.md — detailed plan: small-scope refactors of the ported code (interlude)

**Interlude between Phase 2T (e2e export coverage) and Phase 3 (viewer + layout).**
The port so far (~40k lib LOC) was written to be *behaviorally* faithful — JSON/diagram
bytes are the contract — and the e2e + golden + oracle nets now guard that contract at
unprecedented breadth (313 corpus entries, export matrix, full-tier invariants, 12
corpus benches). 2R spends that safety net: remove **Java-isms in the Rust structure**
that default clippy does not flag, while the bytes it produces stay bit-identical. The
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

## P2R-D4 Lint ratchet (ratified: selective, not pedantic-everything)

After the sweep waves (never before — measure against a clean tree): try each candidate
lint workspace-wide, keep the ones that land with **≤ 10 narrow `#[allow]`s total**
(each allow gets the faithful-port rationale, `agents/notes/process/faithful-port.md`).
Candidates (from the survey checklist, non-binding): `explicit_iter_loop`,
`needless_pass_by_value`, `trivially_copy_pass_by_ref`, `match_same_arms`,
`unnecessary_wraps`, `redundant_clone` (unstable → requires a `NIGHTLY.md` register row
+ measured compile-time hit; drop it if either is bad). Kept lints are enabled as
`#![warn(clippy::…)]` per lib crate + a WORKFLOW.md §Code style row. No lint exists for
accessor naming (`get_x` → `x`) — that pattern is enforced by the survey + review only.

## P2R-D5 Waves (ordered, each ends green — quartet + full tier + bless no-op + commit)

Size gate honored: each wave is ≤ ~4 modules and gets its registry-item list in this
file before starting.

| MS | Deliverable | Exit gate |
|---|---|---|
| PR0 | Close the survey: populate §Register from the four crate sweeps; assign dispositions; record out-of-scope rejects | register complete + dispositions 100 %; quartet green (docs commit) |
| PR1 | `oxidizr-core` wave (model/builder/viewops — biggest clone + accessor cluster; P3 builds directly on it) | green + `bench cmp 2T` reviewed |
| PR2 | `oxidizr-dsl` wave (lexer/parser/fmt — BYTE-PATH heavy: bless no-op + goldens are the boss) | green + `bench cmp 2T` reviewed |
| PR3 | `oxidizr-export` wave (plantuml/mermaid/theme engines; decomposition with citations per D2) | green + `bench cmp 2T` reviewed |
| PR4 | `oxidizr-json` + `oxidizr-api` + `oxidizr-diag` + `oxidizr-ast` + `oxidizr-cli` wave | green (byte-path: json writer per D2 cite rule) |
| PR5 | `xtask` + `oxidizr-e2e` harness sweep (least constrained) | green |
| PR6 | §D4 ratchet measurement + enable kept lints; `bench save 2R` baseline; close-out docs (WORKFLOW §Code style row, PROGRESS board + log, this file DONE, RESUME) | quartet + full tier + `oracle verify` + `bench cmp 2T/2R` sane; **no tag** |

## P2R-D6 Verification list (empirical, before dependent code)

- **V-R1** (before PR1) — invariance harness sanity: run the wave ritual (full tier +
  bless) on an **unmodified** tree and record the no-op — proves the ritual itself is
  meaningful before the first behavior-adjacent edit lands. **CLOSED 2026-11-18**:
  full tier 8/8 ok + `cargo xtask e2e bless` → every entry `unchanged`, corpus git
  diff = 0 lines, at HEAD `7e1fe73` (pre-2R). The ritual is a real net.
- **V-R2** (before PR6) — lint candidate measurement: per-candidate workspace try +
  allow-count tally + (`redundant_clone` only) compile-time delta → decision table
  recorded under §D4.
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

## Register (populated at PR0)

_Survey of 2026-11-18 in flight: dsl / core / export / json+api+cli+misc sweeps —
findings land here as `R-dsl-nn`, `R-core-nn`, `R-exp-nn`, `R-mis-nn` rows with
pattern, evidence (file:line), disposition class, wave._
