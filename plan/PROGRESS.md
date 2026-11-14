# PROGRESS.md — implementation work journal

Living notes for the implementer (survives context loss). Newest entries on top.
Authority order: `DEVIATIONS.md` (contract + Locked) > `build/PHASE_N.md` > this file
(journal/notes only — decisions belong in the phase files).

## Status board

| Phase | Milestone | State |
|---|---|---|
| P1 | M0 scaffold | **done 2026-11-14** (this commit) |
| P1 | M1 ast+lexer | next |
| P1 | M2 … M7 | pending |
| P2 | exporters | pending |
| P3 | viewer+layout | pending |

Commit cadence (user decision 2026-11-14): **one commit per green milestone**, nothing
in between. Questions only on plan-contradicting reality (AGENTS.md working style).

## Environment facts (verified 2026-11-14)

- `cargo`/`rustc` on PATH = flake's pinned nightly: `cargo 1.102.0-nightly (ede6cefea
  2026-11-11)`, `rustc 1.102.0-nightly (8eb778d04 2026-11-12)`. The session PATH already
  carries the dev shell (rustc, cargo, structurizr, java-less oracle wrapper all work).
- Oracle on PATH: `structurizr` → nix-wrapped war 2026.10.26 (library 6.2.4).
  Invocation contract (verified): `structurizr export --workspace F --format json
  --output DIR` (GNU options only; `-o` is a directory; output file = input basename +
  `.json` inside it). Version line: `structurizr: 2026.10.26 / structurizr-*: 6.2.4`.
- Reference clone (Apache-2.0, pinned `ad34036`): `/tmp/nix-shell.uni100/opencode/structurizr`.
  DSL fixtures live in `structurizr-dsl/src/test/resources/dsl/` (84 top-level `.dsl` +
  subdirs `extend/`, `image-views/`, `decisions/`, `docs/`, `big-bank-plc/`,
  `spring-petclinic/`, `include/`… 113 `.dsl` in repo total — vendor only the dsl dir +
  client read-corpus + validation, see P1-D10).
  Read-corpus: `structurizr-client/src/test/resources/` (`backwardsCompatibility/`,
  `structurizr-36141-workspace.json`…). Validation fixtures: same resources dir
  `workspaceValidation/` (16 JSON).
- V-10 fixture (no views): `workspace-properties.dsl`.
- Reference download host works: `download.structurizr.com` (static.structurizr.com does
  NOT resolve — never use in docs).
- git remote: `git@github.com:jiricihelka/oxidizr.git` (nothing pushed yet as of M0).
- python3 is NOT on PATH; use
  `/nix/store/9zclbh8196695bbjs106wh8c4957n002-python3-3.13.12-env/bin/python3`.

## Working notes / errata found while implementing

- (M0) Fixture path correction: `…/test/resources/dsl/…` (notes-*.md listed bare
  filenames) — vendor script must use the `dsl/` root.
- **V-10 RESOLVED (2026-11-14)** — reference app pipeline around every `export`:
  `AbstractCommand.loadWorkspace` = load → `WorkspaceScopeValidator.validate()` (hard
  error; fixtures failing validation can never have goldens) →
  `new DefaultInspector(workspace)` → `createDefaultViews()` if `!model.isEmpty() &&
  views.isEmpty()`.
  - Default views (V-10 proper): generated for **model non-empty + zero views**
    (`workspace-properties.dsl` has an empty model → none, which is why the first probe
    showed nothing). Shape per `structurizr-core/…/view/ViewSet.createDefaultViews()`
    (class renamed from `Views` in 6.2): landscape `addDefaultElements` + per system
    (sorted by name) context & container views + component view per container with
    components + deployment views per environment×system; empty key ⇒ auto-keyed
    (V-4 numbering). **`dsl2json` must mirror this** (app-parity is the oracle contract);
    keep the *library* generation-free (Locked list).
  - Inspection counters: `DefaultInspector` writes `structurizr.inspection.error/.
    warning/.info/.ignore` (violation counts) into workspace `properties` on EVERY
    load/export. Harness artifact, not file content; whole `structurizr-inspection`
    crate is out of scope to reimplement. Goldens strip these four lines
    (`cargo xtask` `normalize_reference_output`, incl. trailing-comma fixup when they
    sort last); json2json round-trips them as ordinary properties. DEVIATIONS-worthy —
    register there if user-visible behavior solidifies in M3/M4.
  - DSL parser defaults: workspace name/description default to the literal strings
    `"Name"` / `"Description"` (`WorkspaceParser.java:28`), overwritten by the
    `workspace` statement. Builder (M4) must start from these.
- **Properties map serialization = Java HashMap order (V-11, M3 blocker)**. Proof:
  workspace-properties export ⇒ `error, dsl, info, dslEditor, warning`; 36141 JSON→JSON
  ⇒ `info, ignore, error, warning, dsl`. Neither sorted nor insertion-ordered; order is a
  function of key hashes (+bucket chains). Plan correction: P1-D5's "BTreeMap sorts for
  free" is WRONG for byte parity of `properties` (any `properties` object: workspace,
  elements, relationships, views, …). POJO fields remain declaration-ordered (the
  golden's alphabetical top level is POJO field order, that part is fine). Options for
  M3 (decide there, recorded in WRITE-CATALOG): (a) replicate Java `String.hashCode` +
  HashMap bucket walk in a custom serializer, or (b) parity comparator canonicalizes
  key order of `properties` objects JSON-aware. (a) is compatible with byte-goldens;
  (b) weakens the gate. Empty object renders `{ }`, empty array `[ ]`, `"key" : value`.
- **Volatility RESOLVED — export is deterministic**: no timestamp is stamped by the app
  (earlier note about `lastModified` was wrong). `lastModifiedDate/. lastModifiedAgent`
  round-trip from JSON inputs verbatim; `lastModifiedUser: ""` omitted (empty-string
  omission); unknown keys (`revision` in cloud exports) dropped; compact input JSON is
  rewritten in Jackson pretty layout; numeric `workspace.id` preserved (36141 test).
  So read-corpus goldens need no normalization.
- `loadWorkspace` also `installThemes`(local `themes/` dir or `$STRUCTURIZR_THEMES`) —
  theme URLs are fetched during PNG/SVG export only, not `--format json`.

## Milestone log

### P1-M0 (2026-11-14, in progress)

Scope (P1-D11 M0): workspace + 7 crates, `rust-toolchain.toml`+`NIGHTLY.md`, LICENSE
files + metadata, `docs/diagnostics.md` stub, lints, CI (3-OS + bench + coverage +
reference jobs), criterion skeleton + committed baseline, xtask oracle v0 (gen ONE
fixture) + `vendor`, testdata vendored with attribution. flake.nix: DONE (9d0498e).

- Pin: rust-toolchain.toml channel `nightly-2026-11-13` == the flake-provided toolchain
  (rustc version string lags the channel date by 1 day: `…(8eb778d04 2026-11-12)`).
  Both bumped together at milestones, register in NIGHTLY.md.
- rust-version = 1.85 (edition-2024 floor; no nightly features used in P1 code so far —
  see NIGHTLY.md).
- `.cargo/config.toml`: `[alias] xtask = "run --package xtask --"` (matklad pattern)
  makes the mandated `cargo xtask …` spelling work; committed (dev tooling, not product
  config).
- Toolchain quirk (nightly-2026-11-13): cargo now emits manifest-level `unused
  dependency` WARNINGS for declared-but-unused deps (skeleton crates; P1-D9 set declared
  up front). Not errors — `-D warnings` unaffected; each retires when its milestone
  lands the code. Skeleton libs carry `#![allow(unused_crate_dependencies)]` with
  remove-when notes for the rustc lint side.
- xtask fixes made while running it for real: `Path::extension()` has no dot (walk_files
  `.dsl` comparison — counted 0 fixtures); `scan_deferred` must be lossy (iso-8859.dsl
  is not valid UTF-8, V-8).
- Clippy surfaces that needed the honest fix: clap default rename would be
  `dsl2-json` → `#[command(rename_all = "lowercase")]` on the `Command` enum (surface
  names are Locked); `doc_markdown` wants backticks around `PHASE_1`, `NO_COLOR` etc.
- Vendored (107 `.dsl` + 21 JSON): deferred-for-D7 list is exactly
  `dsl/big-bank-plc/model/internet-banking-system/details.dsl`, `dsl/decisions/workspace.dsl`,
  `dsl/docs/workspace.dsl`, `dsl/include/model/software-system/model.dsl` (cross-checked
  vs raw grep). Validation corpus = 16 JSON. read-corpus workspaces carry round-tripping
  `lastModifiedDate*` (deterministic — no golden normalization needed).
- V-10 golden: `testdata/golden/workspace-properties.json` generated through the oracle
  (normalized: 4 `structurizr.inspection.*` lines removed, last-entry comma fixed —
  exercised for real: they DO sort last here) + `cargo xtask oracle verify` → ok.
- M0 gates: fmt / clippy(-D warnings) / test all green on the pinned toolchain; bench
  baseline committed under `bench-baseline/M0/`.
