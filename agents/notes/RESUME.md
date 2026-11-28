# RESUME — session checkpoint (updated 2026-11-28, **PHASE 6 IN FLIGHT — GUI: arrow model-editing + help texts. M0+M1+M2+M3+M4+M5 DONE (dep spike, binding table, session history, route UX, headless ModelCommand core, canvas model ops, details panel — gates green each); NEXT = M6 help overlay (F1/? table from shortcuts.rs + tooltips + V6 audit). Previous arc: P5R CLOSED 2026-11-26, tag `v0.5.0` LOCAL. Never pushed — push/publish stay the user's call**)

## Where P6 stands

- Plan: `agents/plan/build/PHASE_6.md` (Status PLANNED; decisions P6-D0…D8,
  waves M0…M8, verification V1…V9, risks). Registry rows F13+F14 in
  `agents/plan/ADDITIONAL_FEATURES.md`; deviation **D17** (single session
  history vs the layout editor's per-diagram undo) in `agents/plan/DEVIATIONS.md`.
- User rulings anchoring everything (2026-11-28): full scope accepted (route
  UX + model editing + help overlay + details panel + markdown/docs);
  **single session history**; create = **port-dot drag**; **Del deletes the
  edge**, Ctrl+Shift+K strips waypoints; **egui_commonmark allowed** (0.26 =
  egui ^0.37, verified on crates.io; add behind `gui`, `default-features =
  false`, no image loading); **model edits BLOCKED for DSL sources** —
  plain `.json` only, sidecar-preferred reopens stay blocked (gate fn per
  P6-D4; the DSL file is never rewritten).
- Work autonomously per the wave table; commit every green wave-piece;
  questions ONLY on plan-contradicting reality (AGENTS.md style).

## Implementation anchors (measured 2026-11-28, P5R tree)

- Viewer = headless lib core + eframe/egui **0.37** shell behind `gui`;
  shell split `shell/{mod,canvas,actions,panels,widgets,browser}.rs`.
  ALL edits funnel through `Viewer::edit_current_view` + `commands::History`;
  `Command` mutates only `LayoutOverlay` (P3-D4, amended by P6-D2).
- `Viewer.workspace` is `Option<Workspace>` (owned — M3 landed the model
  commands mutating it directly) → `run_model_edit` +
  `rebuild_scene()`; new ids via
  `oxidizr_core::ids::IdGenerator` re-registered from all ids (`found()` then
  `generate()` — the JSON reader's own dance).
- Save: `shell/actions.rs::save_workspace` clones the workspace + applies
  overlays through the SHARED `oxidizr_json` writer; add the `model_dirty`
  leg per P6-D8 (save fires on model-only dirt; filtered-view block stays
  for geometry-only saves).
- Today's arrow gestures live in `shell/canvas.rs` (waypoint insert on
  dblclick, drag, Alt+press delete, Del strip — the Del meaning flips in
  M4); key handling in `shell/mod.rs::ui` (focus guard `text_focus`);
  tooltips `shell/widgets.rs::tooltip_text`. There is NO keybinding help
  anywhere today — `shortcuts.rs` (M0) becomes the single binding table.
- egui_commonmark 0.26 (API verified in the vendored registry source):
  `CommonMarkViewer::new().show(ui, &mut CommonMarkCache, text)` — ONE
  shared `CommonMarkCache` on `Viewer` (the crate keys its internals per
  widget id; the content-keyed capped `Markdown` objects were the OLDER
  API — nothing to cap); `load-images` OFF (icons have their own pipeline).

## If a next session opens

- **M0+M1+M2+M3+M4+M5 DONE 2026-11-28** (evidence in the PHASE_6 wave rows):
  details panel LIVE (`shell/panels.rs`): `selection_details()` builds an
  owned `Details` snapshot (headless rows + `key::element` model facts +
  edge `EdgeFields`), `info_block` renders element tags/properties/
  MARKDOWN description (`Viewer.md` shared `CommonMarkCache`) + URL
  hyperlink; an edge on a `.json` source gets Description/Technology/
  ViewLabel fields — one `field_draft` at a time, commit = ONE
  `run_model_edit` on Enter/focus-loss/field-vanished (`field_seen`
  prune in `key_panel`), `ViewLabel` keyed by `(view_key, id)`.
  Headless twin: `ModelCommand::SetRelationshipViewLabel` +
  `set_relationship_view_label` factory + round-trip test; `key::element`
  lookup + test. ALL interim `allow(dead_code)` marks are gone (only the
  crate-level headless guard in `lib.rs` remains). 127 viewer lib tests.
  Start wave **P6-M6** (help overlay): F1/`?` opens a centered
  `egui::Window`/`Area` rendering `shortcuts::all()` sections; Esc closes
  help BEFORE clearing selection (priority in `canvas_keys`); the
  empty-canvas hint renders the same table; V6 audit = every table row
  vs the actual handlers (the table is already current from M0); add
  tooltips from the table where gestures are discoverable-poor.
  Manual smoke (plain `.json`, run before close, M4+M5 lists):
  port-drag create + select lands, Ctrl+Z trace-free, endpoint
  reconnect, Del + restore, DSL source = no dots/squares + blocked note,
  model-only save from a filtered view, chip clears on save; element
  markdown/tags/props/link render; edge field Enter commits one undoable
  step, ViewLabel changes only the current view's label.
- Gate per commit: `cargo xtask ci` quartet AND `cargo xtask ci gui fmt
  clippy test refs` (every viewer/gui commit); bless NO-OP tripwire always.
- Windows-target watch: M4+ touches filesystem only through the existing
  save path; run `cargo xtask ci windows` at close (V9) — pitfalls note
  `windows-path-separator-assertions.md` still governs any NEW fs test.
- Tags `v0.1.0…v0.5.0` all LOCAL — pushing them is the user's act. P6 does
  NOT close with a tag unless the user asks.

## Workflow (measured, `agents/notes/process/parallelism.md`)

- NO subagents by default (measured memory thrash; hard cap 3). Parallelize
  only harness cycles via background shell jobs. Two cargo jobs queue on the
  target-dir lock — chain in one background command; don't poll background
  shells (`background: true` as a BOOLEAN, string "True" is rejected).
- Script hygiene: python to a file under the session tmp dir, never
  `python3 -c` with backticks/`\b` (pitfall `agent-shell-quoting.md`); regex
  replaces MUST use f-strings for `{name}`; the nix dev-shell banner prefixes
  redirected stdout — strip to the first `{` before JSON parsing; `od`
  slicing must account for it (it ate a fmt-bug repro once).
- Gate-log reading: `grep -c` exits 1 on zero matches — printed `0`/
  `FAILED 0` is GREEN; verify `ci: all steps green` in the log body.
- Never edit tracked files while a gate/bench/bless chain runs; journal
  drafts go to the session tmp dir; apply after the chain, re-run `refs`.
- GUI-leg drift: `gui-leg-lint-drift.md` — the quartet never compiles the
  shell, so a plain `ci` passing does NOT mean the gui leg does.

## Gates quick reference

- Sub-piece: quartet green → commit (`cargo xtask ci` in `nix develop`).
- Viewer/cli-gui/dsl-touching commits: `cargo xtask ci gui fmt clippy test refs`.
- Waves: + `OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e` + bless NO-OP
  (307×unchanged, `git status --porcelain testdata/` empty) +
  `cargo doc --workspace --no-deps` at 0 + `bench cmp <LABEL>` (run
  `cargo xtask bench` in the same session first).
- Category B env: `$OXIDZR_STRUCTURIZR_BIN=/nix/store/
  5yil7chk3cjg3qmsabq1vzdxv0v4bwm0-structurizr-2026.10.26/bin/structurizr`
  (dev shell exports it), legs `TZ=UTC`, background only.
- Windows leg: `cargo xtask ci windows` (Wine, dev shell; full suite, slow).
- `dist plan` = `nix develop --command dist plan` (not an xtask subcommand).
- Bench baseline `5R` stands unless P6 close saves a new label.
