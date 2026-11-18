# PHASE_2T.md — detailed plan: e2e coverage for the Phase 2 export features (interlude)

**Interlude between Phase 2 (v0.2.0) and Phase 3.** The P1T net (156 valid + 144
invalid/mutated entries, manifests = exit class + OXZ code set + counts, never bytes,
never messages) exercises `dsl2json`/`json2json`/`validate`/`fmt`/`reconcile` — but
was cut before `export` existed: **no corpus entry ever runs an export**. The P2 byte
gates (testdata goldens, witness sweeps, oracle legs) cover the *selected* fixture
matrix, not the corpus breadth — a refactor breaking export on exotic-but-valid
workspace shapes slips past every quick-tier test. 2T closes exactly that gap, reusing
the P1T machinery (API seam, bless, tiers, benches) without changing it.

Locked premises (user decisions precede this file): same assertion philosophy as P1T
(PHASE_1T.md header); **no behavior change** — every fix triggered by 2T is a real P2
bug fixed with a witness/oracle verdict, never a bless-around; `testdata/` untouched;
**no tag at close** (1T precedent: no shipped-behavior change); quick tier stays
< 60 s, heavy invariants ride the existing nightly `e2e.yml`.

## P2T-D1 API seam — unchanged (this is a design result, not laziness)

`Request::Export { workspace: PathBuf, format: String, output: Option<PathBuf> }`
landed with P2 (CLI is already a shim over it); fs side effects are confined to the
explicit `output` dir — exactly the `FmtMode::Write` exception class from P1T-D1.
No new variants, no `Outcome` change: the runner reads emitted diagram files back from
its scratch dir. Export needs a FILE path (reference `ExportCommand` has no stdin),
which corpus entries already are — no scratch copy for the input, only for output and
for invariant replays (P1T-D6 technique).

## P2T-D2 Manifest schema v2 — `export` sections + fan-out

- Sidecar gains per-format sections, matrix = `plantuml`, `plantuml-c4`, `mermaid`
  (the three landed diagram formats; workspace-level `json`/`theme` exports are covered
  by determinism invariants only — same writer functions as `dsl2json`, per-entry
  manifests would assert nothing new):

  ```json
  "export": {
    "plantuml":    { "exit": "ok",    "codes": [], "diagrams": 4 },
    "plantuml-c4": { "exit": "ok",    "codes": [], "diagrams": 4 },
    "mermaid":     { "exit": "error", "codes": [5007], "diagrams": 2 }
  }
  ```

- Applicable ⇔ the entry parses and has ≥ 1 view (both ways checked, 1T rule: missing
  section on a view-bearing entry fails; section on a viewless one fails). Input
  selection mirrors `validate`: `workspace.dsl` if present, else `workspace.json` —
  dsl↔json equivalence is an invariant (D4), not a duplicate manifest.
- `diagrams` = count of output files matching the format extension in the scratch dir
  (naming per `oxidizr-export::naming`); **counts only** — no per-diagram structure in
  manifests (that lives in invariants; bless diffs stay reviewable).
- Unsupported view-kind × format combos record whatever the blessed reality is
  (witness-verified in P2): the manifest records the FULL sorted code set, e.g.
  a deployment view under `mermaid` blessing [5007] — no guessing in fixtures.
- `cargo xtask e2e bless [FILTER]` extends the generator/comparator; export fan-out
  is `thread::scope` per entry×format into per-entry scratch dirs (`/tmp` root,
  removed at runner end — hermetic, no corpus writes).
- Themes: if an entry carries `themes/` (installed-theme layout `<dir>/<name>/theme.json`),
  the runner sets `ApiOptions.themes_dir` to it (and `env: Some(empty)` per the purity
  contract — process env is never read). `allow_net` stays false for all corpus runs.

## P2T-D3 New fixtures — the export axis (16x), authored originals

≈ 10 entries (numbers continue after 159): deployment + dynamic views in export
position (drives the 5007 combos), filtered-view export, legend on/off pair,
`automaticLayout` block set (dialect header differences), deep styles incl.
relationship styles (legend content), installed theme via `themes/` dir, theme URL
offline (exit error [5001] recorded in the export sections; entry itself parses
clean), unknown theme name (P2-D3 interim rule: still works), CJK/quote/newline names
(dialect escaping). Mutations: none — export errors are post-parse by construction.

## P2T-D4 Coverage gate shrink

5007 (export kind×format combos, per PT2 fixtures) leaves the allow-list — the T3
registry recorded it as unreachable, the export matrix makes it reachable. 5001 is
already covered (T3: `!include https://…`); the theme-URL fixture adds breadth on the
theme-load path (a different trigger of the same code — registry reason gains the
second trigger). 5005 (`IMPORT_DEFERRED`) stays allow-listed (deferred importers,
unchanged reason). Gate = the same `tests/coverage.rs` test, no schema change.

## P2T-D5 Invariants (full tier, `tests/full_export.rs`)

Self-consistency only — immune to future blessed deviations (P1T-D6 philosophy):

- **Determinism**: export the same entry twice → identical file set + identical bytes
  (dialects must be clock/random-free; hermetic options pinned).
- **Path parity**: `export(dsl) ≡ export(json2json(dsl))` per output file — the export
  twin of P1T's round-trip byte-stability; catches DSL-vs-JSON engine drift the way
  the color-normalization bug was caught in T4 (a violation here is a REAL bug: fix
  the tool with a witness verdict, never bless).
- **Fixpoint parity**: `export(fmt(x)) ≡ export(x)` via the scratch-dir context replay
  (children copied; `!include`/`extends` resolve).

## P2T-D6 Benchmarks

`e2e_export_{plantuml,plantuml_c4,mermaid}_all` + biggest-entry variants over the
corpus, behind the existing `e2e-bench` feature (CI unaffected — V-T5 precedent);
`cargo xtask bench save 2T` baseline committed (the P2 M4-baseline testdata benches
stay — corpus benches add the 156-workspace breadth).

## P2T-D7 Milestones (ordered, each ends green — quartet + commit + RESUME)

| MS   | Deliverable                                                              | Exit gate |
| ---- | ------------------------------------------------------------------------ | --------- |
| **PT1** | **DONE 2026-11-18** — manifest v2 (`ExportReport`/`ExportReports`, strict parse: 4 format keys required, unknown rejected; diff both ways); runner `observe_export` fan-out (plantuml/plantuml-c4/mermaid + websequencediagrams compat probe) into `scratch_dir`/`export_capture`; coverage gate collects export codes, **5007 off the allow-list**; `tests/path_leakage.rs` (V-T7, permanent); full bless: **157 entries gained export sections** (156 valid + invalid/09-reconcile-contested), 155 carry 5007, 2 carry 5001 (theme-URL entries `75-views-theme`, `158-composite-full-kitchen` — the reference theme-load-before-dispatch order), mutations untouched. Landed refinements: `files` counts ALL scratch files (legend/frame name parsing unsafe for keys ending `-<digits>` — deviation from the draft's `diagrams`, documented in manifest docs); 5007 coverage arrived with the compat-probe slot earlier than PT2 | bless green; quick tier **~0.1 s** with fan-out (V-T6 closed — budget 60 s); V-T7 green; quartet + full tier green |
| **PT2** | **DONE 2026-11-18** — 7 authored fixtures 160–166: installed theme via `themes/acme/theme.json` (160 — the reference `<workspace-parent>/themes` default source makes the planned `themes_dir` runner hook UNNECESSARY); broken theme file → **5008 per format, ahead of 5007** (161, reference load-before-dispatch order); unknown theme name silently skipped (162 — **JSON-only**: DSL `theme <unknown>` hits the FILE branch → parse-time statement error, the load-skip branch is DSL-unreachable); terminology + container view (163, `typeOf`); autoLayout positional variants — grammar takes `tb|bt|lr|rl` + separations + vertices (164); CJK/quotes/«»/brackets/backslashes/multiline escapes (165 — view keys are `[a-zA-Z0-9_-]`, dots rejected by the reference charset); **all 8 view-kind statements + 2 deployment environments in one clean workspace** (166 — biggest clean export: 14 plantuml files). Coverage: **5008 off the allow-list**; 5009 verdict = permanent (probed dangling-elementView/dynamic/kind-mismatch shapes on JSON feeds: hydrate+validate reject with 4001 pre-dispatch). Plan-axis deviations recorded: deployment/dynamic 5007 combos don't exist (only `websequencediagrams` raises it — PT1 bless); legend on/off has no workspace axis (format-driven, baked into blessed counts); theme-URL-offline = `theme default` → mapped URL, already covered by 75/158 | bless green; gate green; second bless all-unchanged; quartet + full tier green |
| **PT3** | **DONE 2026-11-18** — `tests/full_export.rs` (full tier): determinism (double capture), **path parity** `export(dsl) ≡ export(json2json(dsl))`, fmt-fixpoint parity `export(fmt(x)) ≡ export(x)` — per entry × landed format over all 163 export-applicable valid entries; replays run from a scratch COPY of the entry dir (children + `themes/` — theme chain + include/extends see what the original input sees; unit-tested). **V-T8 CLOSED pre-wiring: 9/9 probes byte-exact parity** (166 + 03 + 95 × 3 formats) — no P2 bug, invariant landed green as required; vacuity guard (`!entries.is_empty()`) | full tier green locally; windows leg green (PT3b); dispatched `e2e.yml` run queued for PT5 |
| **PT3b** | **DONE 2026-11-18** — Windows CI fix (user-reported): `subprocess_smoke::binary()` walked ancestors for `oxidizr` only — native `windows-latest` ci.yml leg (P1T-era latent red, surfaced when the 2T CI ran): the bin there is `oxidizr.exe`, so resolution failed regardless of the build. Fix cfg-free: every ancestor tries BOTH `oxidizr` and `oxidizr.exe` (charter: no `#[cfg]` without need); `$OXIDZR_BIN` precedence unchanged | windows leg green (CI re-run); Linux quartet green |
| PT4  | Corpus export benches + `2T` baseline commit                             | `bench cmp 2T` works; CI bench job still feature-free |
| PT5  | Close-out: TESTING.md §E2E (manifest v2 + export matrix), e2e/README, PROGRESS board + log, this file's DONE entries | quartet + actionlint green; **no tag** |

Size gate: 4 modules (oxidizr-e2e, xtask, e2e/corpus, docs) — this file is the
decomposition regardless.

**PT1 catch report (the net paying for itself):** the full tier surfaced two
PRE-EXISTING P2 regressions, both bisected to `cc243a0` (M2-d part 2) and fixed per
the faithful-port rules, not bless-arounds: (1) `reconcile` had never received the
M1-e-B `createDefaultViews` load tail — self-reconcile dropped every generated view
(fixed `726a724`: tail on both reconcile sides, the reference loads both through
`AbstractCommand.loadWorkspace`); (2) the P1T `json2json` byte-stability invariant
became false by contract once the JSON reader grew the reference TreeSet re-sort
(WRITE-CATALOG §5, oracle witnesses) — redefined as the byte **fixpoint** `out₃ ==
out₂` (`2cb3215`), which keeps the byte-exact drift catch and encodes the real
contract.

## P2T-D8 Verification list (empirical, before dependent code)

- **V-T6** (before PT1) — **CLOSED 2026-11-18**: the whole quick tier (313 entries,
  full export fan-out included) runs in ~0.1 s in-process — no subset split needed,
  the matrix covers the full corpus in the quick tier. Budget 60 s, headroom ×600.
- **V-T7** (PT1) — **CLOSED 2026-11-18 as a permanent test**: `tests/path_leakage.rs`
  exports three deterministic entries × two formats into two distinct scratch dirs
  and asserts byte identity + path-free file names (rides the quick tier).
- **V-T8** (before PT3) — path parity pre-probe on 3 diverse entries (views + styles
  + properties): if `export(dsl) ≠ export(json2json(dsl))` anywhere, that is a P2 bug
  → witness verdict → fix before wiring the invariant (an invariant must land green
  or not at all).

## Risks

- **Blessing wrong reality**: export sections are blessed from runs, and the P2
  witness sweeps (m2d/sweep) are the compat backstop — a blessed exit/code set that
  contradicts the oracle is a bug in oxidizr, fixed per the faithful-port rules, NOT
  re-blessed.
- **Quick-tier bloat** (V-T6 mitigates; the runner stays parallel; export per entry is
  ms-scale by the M4 benches).
- **Scratch-dir flakiness** (parallel fs under `/tmp`): unique dir per entry×format,
  cleanup in the runner's scope-drop, no shared state — V-T7 doubles as the canary.
- **Corpus export = DSL-path bias**: mitigated by the path-parity invariant (D5),
  which makes every valid entry a JSON-path export test nightly.
