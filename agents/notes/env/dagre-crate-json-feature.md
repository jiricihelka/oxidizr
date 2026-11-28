# dagre 0.1.2 — the `json` feature cannot be turned off

Measured 2026-11-28 while wiring `oxidizr-layout`'s `dagre` feature
(P7-M0, `agents/plan/build/PHASE_7.md` §M0 measured reality).

* `dagre = { default-features = false }` **fails to compile**: `graph/json.rs`
  is a plain `pub mod` that unconditionally `use`s `serde` — the `json`
  feature gates nothing. Six E0432/E0659 errors. Upstream defect (filed
  nowhere; the crate is 0.1.2, one release).
* Fix: `features = ["json"]` (workspace `Cargo.toml` carries the comment).
  Cost: serde/serde_json — already in the viewer's tree, so `cargo tree`
  delta vs the viewer = **`dagre` only** (`dagre` + `log` genuinely new, the
  serde pair unifies).
* Measured trees: default `-p oxidizr-layout` = EMPTY; `--features dagre` =
  14 crates, zero windowing/GPU. MSRV fine (edition 2024).
* Determinism audit (source): layout phase uses BTreeMap/BTreeSet + stable
  sorts; graph keys live in `HashMap`s BUT iteration goes through
  insertion-order `node_order`/`*_edge_order`/`preds_order` vectors; no
  rand/time/rayon. Dummy names come from a monotonic global `AtomicU64`
  ⇒ in-process repeats are a hazard axis DISTINCT from cross-process runs;
  `tests/dagre_determinism.rs` covers both (10 concurrent children via
  `current_exe()` + `--exact`, env `OXIDZR_DAGRE_DETERMINISM_CHILD`).
* Heavy corpus shapes cost ~10 s/battery in debug — `Sweep::Directions`
  halves the big fixtures; keep the determinism test near ~15 s.
* Related trap surfaced the same day: `oxidizr-layout --features dot-backend`
  was broken since the P5R visibility demotion (`e76ba12`: `pub(crate)`
  consts still on the `pub use` line ⇒ E0364) — no CI leg enables that
  feature, so nothing caught it. `cargo check -p oxidizr-layout --features
  dot-backend,dagre` is the local guard until a CI leg exists.
* `#[cfg(feature = …)]` on a `use`/statement is fine, but a hygiene rule to
  remember: shipped prose must not cite process ids (refs leg).

Last useful: 2026-11-28
