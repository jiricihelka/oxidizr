# RESUME — session checkpoint (updated 2026-11-28, **PHASE 6 IN FLIGHT — GUI: arrow model-editing + help texts. M0…M7 DONE (dep spike, binding table, session history, route UX, headless ModelCommand core, canvas model ops, details panel, help overlay, documentation window — gates green each); NEXT = M8 CLOSE (CHANGELOG review, viewer README + docs/ refresh incl. the README gesture list from the table, PHASE_6/PROGRESS/RESUME journals, full V-list: ci windows + Category B oracle TZ=UTC + doc-zero + V4 tree; manual smoke lists of M4+M5+M6+M7 rows; NO tag). Previous arc: P5R CLOSED 2026-11-26, tag `v0.5.0` LOCAL. Never pushed — push/publish stay the user's call**)

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

- **M0…M7 DONE 2026-11-28** (evidence in the PHASE_6 wave rows): canvas
  model ops + details panel + help overlay + documentation window all
  LIVE. Key pieces: press chain waypoint → insert dot → endpoint handle →
  port dot (gated) in `press_mode`; `Viewer::run_model_edit` funnel; gate
  `commands::model_editing_allowed`; `PaintOptions.model_editing`; Del =
  delete + Ctrl+Shift+K strip; `model_dirty`/`model_note` + save rules.
  Details panel: `selection_details()` → `Details` snapshot → element
  facts/markdown (`Viewer.md` `CommonMarkCache`)/link + edge fields
  (`field_draft` one-at-a-time, ONE commit per Enter/focus-loss/
  vanished-field, `ViewLabel(view_key, id)`). Headless twin:
  `SetRelationshipViewLabel` + `set_relationship_view_label` +
  `key::element`. Help: `Toggles.help`, `help_window` renders
  `shortcuts::all()` (F1 / Shift+`?` / `?` chip; Esc closes help BEFORE
  the selection clear), `affordance_hint` tooltips, empty-canvas hint
  looks up the F1 row. Docs: `overview::doc_sections` (workspace-level →
  attached → embedded, each `order`-sorted) + `has_element_docs` gate;
  `Toggles.docs` + `docs_window` over an owned `DocPage` snapshot
  (markdown via `Viewer.md`, `AsciiDoc`/absent verbatim + note); F2
  handler AND its table row landed together as promised; Documentation
  button in `info_block`. 129 viewer lib tests.
  Start wave **P7 = P6-M8 (CLOSE)**: CHANGELOG review, viewer README +
  `docs/` refresh (README gesture list renders from the table), PHASE_6 +
  PROGRESS + RESUME journals, FULL V-list — V4 `cargo tree` unchanged,
  `cargo xtask ci windows`, Category B oracle `TZ=UTC`
  (`$OXIDZR_STRUCTURIZR_BIN`), doc-zero, bless; run the manual smoke
  lists of the M4/M5/M6/M7 wave rows BY HAND; NO tag.
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
