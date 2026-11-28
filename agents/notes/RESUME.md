# RESUME — session checkpoint (updated 2026-11-28, **PHASE 6 CLOSED — GUI: dependency-arrow editing + help texts. M0…M8 all DONE (plan `9c4a4bf`, waves `a73a85a`…`e4d6606` + close); V9 chain ALL GREEN incl. windows/Wine + Category B oracle + export-verify TZ=UTC. NO tag at this close. AWAITING USER DIRECTION for the next phase (no P7 plan exists yet). Never pushed — tags `v0.1.0…v0.5.0` and all P6 commits are LOCAL; push/publish stay the user's call**)

## Where things stand

- **P6 closed 2026-11-28.** Charter, decisions P6-D0…D8, wave evidence and
  the V1…V9 results: `agents/plan/build/PHASE_6.md` (Status CLOSED); board +
  milestone log: `agents/plan/PROGRESS.md`; user-facing summary:
  `CHANGELOG.md` `[Unreleased]`.
- Shipped in P6 (all viewer, `gui` feature): route-editing kit (insert dots,
  endpoint squares, nudges, Ctrl+Shift+K), model editing on plain `.json`
  ONLY (port-drag create / reconnect / Del delete / relabel — gate
  `commands::model_editing_allowed`, the DSL file is never rewritten), ONE
  session undo history (D17), help overlay from the single `shortcuts.rs`
  table (F1/`?`), details panel (facts + markdown + edge fields), docs
  window (F2, `doc_sections`, markdown via egui_commonmark + verbatim
  fallback). Headless viewer tree still carries ZERO windowing crates (V4).
- **User-side pending:** the manual GUI smoke lists in the M4/M5/M6/M7 wave
  rows of `PHASE_6.md` — interactive checks (drag a port dot, edit fields,
  F1/F2 windows) an agent session cannot run. Everything automatable is green.
- **No new phase is planned.** If the user asks to continue the GUI arc
  (e.g. element CRUD, AsciiDoc rendering, per-view history toggle), plan it
  into `agents/plan/build/PHASE_7.md` first — decisions get registered, per
  the charter.

## Implementation anchors (what P6 left behind, current tree)

- Viewer = headless lib core + eframe/egui **0.37** shell behind `gui`;
  shell split `shell/{mod,canvas,actions,panels,widgets,browser}.rs`.
  Geometry edits funnel through `Viewer::edit_current_view`, MODEL edits
  through `Viewer::run_model_edit` (gate → factory → `History::run_model` →
  `model_dirty` → rebuild + repaint); ONE `commands::History` for the
  session, steps tagged with the affected view key.
- Model commands are invertible `ModelCommand`s mutating the owned
  `Viewer.workspace`; new ids via `oxidizr_core::ids::IdGenerator`
  re-registered from all ids (the JSON reader's own dance). Witnessed
  shape: a created relationship stores EMPTY tags (derived `Relationship`),
  description ABSENT.
- `shortcuts.rs` is the single binding table — the help overlay renders it
  verbatim and the empty-canvas hint looks its row up. A new binding lands
  in the table IN THE SAME commit as its handler (V6 posture).
- Markdown: `CommonMarkViewer::new().show(ui, &mut self.md, text)` with the
  ONE shared `CommonMarkCache` on `Viewer` (egui_commonmark 0.26,
  `load-images` OFF, gui-only). Windows that render markdown through it
  gather an OWNED snapshot first (the `DocPage`/`Details` posture) — the
  closure needs `&mut self.md`.
- Save: `shell/actions.rs::save_workspace` clones + applies overlays
  through the SHARED `oxidizr_json` writer; fires on model OR geometry
  dirt; `testdata/` is the byte contract — bless stays a NO-OP.

## If a next session opens

- Read `agents/plan/PROGRESS.md` status board + this file; wait for the
  user's scope. Do NOT invent a phase — planning is question-driven per
  AGENTS.md.
- Gate semantics trap (paid at P6 close, pitfall
  `xtask-ci-steps-replace-defaults.md`): `cargo xtask ci <STEP…>` REPLACES
  the default quartet. The commit gate is the FULL form —
  `cargo xtask ci gui fmt clippy test refs` (every leg named). A bare
  `ci gui` is a clippy-only fast re-check, NEVER a gate.
- Shipped-text hygiene (same hunt): bare F1..F19 in `crates/oxidizr-*/src`
  comments = "feature tag" violation; the self-gating xtask tests run in
  the `test`/`refs` legs — `agents/notes/pitfalls/refs-hygiene-prose-patterns.md`.

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

- Sub-piece: quartet green → commit; anything touching the shell/gui:
  `cargo xtask ci gui fmt clippy test refs` (FULL form, see trap above).
- Waves: + `OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e` + bless NO-OP
  (307×unchanged, `git status --porcelain testdata/` empty) +
  `cargo doc --workspace --no-deps` at 0 (RUSTDOCFLAGS="-D warnings") +
  `bench cmp <LABEL>` (run `cargo xtask bench` in the same session first).
- Category B env: `$OXIDZR_STRUCTURIZR_BIN=/nix/store/
  5yil7chk3cjg3qmsabq1vzdxv0v4bwm0-structurizr-2026.10.26/bin/structurizr`
  (dev shell exports it), legs `TZ=UTC`, background only. Close legs:
  `cargo xtask oracle verify` + `cargo xtask export-verify`.
- Windows leg: `cargo xtask ci windows` (Wine, dev shell; full suite, slow —
  it runs the FULL `cargo test --workspace`, which is where the hygiene
  self-tests live).
- `dist plan` = `nix develop --command dist plan` (not an xtask subcommand).
- Bench baseline `5R` still stands (P6 added no hot-path work worth
  re-baselining; bless + full tier covered the touched paths).
