# RESUME — session checkpoint (updated 2026-11-19, **P2R COMPLETE → P3-M0 next**)

## State

- **P2R is COMPLETE (2026-11-19).** Waves PR0–PR6 all DONE; exit gate green:
  quartet + full tier 8/8 + bless corpus-diff=0 + `oracle verify` 4 json +
  90 diagram + 6 theme + `bench cmp 2T` 23/23 faster-or-equal. **No tag,
  nothing pushed.** Wave hashes + verification recap: PROGRESS.md §Milestone
  log "P2R"; register/waves/§D4 V-R2 table: `agents/plan/build/PHASE_2R.md`
  (status COMPLETE in its header). Code commits: waves through `1c3fcd9`
  (PR5 close), `d890ef9` (PR6-a ratchet), `6baa7b4` (2R baseline), then the
  docs close (this file's commit).
- **Next: P3-M0** (`agents/plan/build/PHASE_3.md` — ratified plan, P3-D0..D10 +
  feature matrix final). FIRST STEP before any code: **M0 touches >4 modules →
  decompose it in `PHASE_3.md` §P3-D9's M0 entry** (charter rule). M0 scope
  (P3-D1/D9):
  1. relocate the reconcile engine `cli` → `core::reconcile` (pure model ops,
     tests move with it, CLI bytes identical — e2e corpus proves);
  2. `oxidizr-layout` crate scaffold — pure graph, ZERO deps (surface per
     P3-D5: `LayoutGraph`/`LayoutResult`/`trait LayoutEngine`);
  3. `oxidizr-viewer` scaffold — lib core modules (`scene`/`hit`/`commands`/
     `reload`, import NO eframe — P3-D1 boundary) + thin eframe bin;
  4. CI: 3-OS headless viewer build (check/clippy all targets, no GUI runs);
  5. `core::filter::eval` if missing (P3-D3) + the P3-D2 one-sentence AGENTS.md
     unsafe-rule touch-up.
  Then the P3-V1..V8 verification items ride before their dependent code
  (oracle + monorepo `ad34036` reads — see `agents/notes/env/toolbox.md`).
- **NETWORK works** (verified 2026-11-19: DNS resolves, crates.io answers — the
  old "NO DNS" note was WRONG; toolbox.md corrected). `eframe`/`notify`/`usvg`/
  `dark-light` are NOT in the local cargo cache; first `cargo add` fetches them.
- Toolchain nightly-2026-11-13. `redundant_clone` (nursery) is now a
  **workspace-wide lint**: write moves, don't last-use-clone, or CI dies; if it
  ever stabilizes, drop the `NIGHTLY.md` row + the `Cargo.toml` line together.
- Ritual unchanged: quartet (`cargo xtask ci`) every commit; engine-touching
  commits also full tier (`OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e`,
  8 `test result: ok`) + `cargo xtask e2e bless` → `corpus-diff=0`; wave-close
  `bench cmp <baseline>` review; `oracle verify` at phase close/weekly; commit
  every green sub-piece + rewrite this file; **never push**; tags only at
  P3-M5 (v0.3.0).
- P2R durable harvest: `agents/notes/process/refactor-patterns.md` (zero-behavior
  playbook); measurement traps: `agents/notes/pitfalls/shell-timing-and-cargo-clean.md`.

## P3 orientation (read first)

- `agents/plan/build/PHASE_3.md` decisions already ratified: egui/eframe+wgpu
  (D2), own `Layered` layout engine + optional `dot-backend` (D5), scene
  architecture (D3), overlay+commands editing (D4), feature matrix final (D0/
  P3-F). Editor rule: layout JSON values only, never the `.dsl` model.
- Byte contract still king: viewer saves go through the `oxidizr-json` writer —
  WRITE-CATALOG rules unchanged, no new serializer (D4); M3 ships an oracle
  round-trip check (D8 Category B).
- New-crate rules: no `unsafe` in OUR code (GPU-dep internals exempt, D2);
  std-only applies to `oxidizr-layout` (zero-dep promise); viewer deps are the
  agreed P3 set only.
