# Scene rebuild + paint budgets (P3-M5, measured 2026-11-20)

Headless probe: `crates/oxidizr-viewer/tests/perf_scene.rs` (ignored tests) —
`open` → `scene::build` → `paint::paint_scene`, release profile, this Linux
box, ROUNDS = 5, 1920×1080 fit viewport. Re-run:

```sh
cargo test --release -p oxidizr-viewer --test perf_scene -- --ignored --nocapture
```

Figures (3 suite runs; paint column = min–max across runs):

| corpus (largest view) | views | items+edges | rebuild ms/view | paint ms/frame |
|---|---|---|---|---|
| big-bank-plc (Components) | 7 | 11 + 13 | 0.026–0.030 | 0.92–1.86 |
| spring-petclinic (Container-001) | 3 | 3 + 1 | 0.002–0.004 | 0.05–0.10 |
| amazon-web-services (deployment) | 1 | 4 + 3 | 0.010–0.024 | 0.09–0.20 |

Budget (§P3-M5, informational per P3-D0): 60 fps (16.7 ms) at ≥ 200 visible
elements. The corpora top out at 24, so `perf_paint_budget_200_elements`
tiles big-bank-plc's Components view into a grid: **209 items + 247 edges →
≈ 1.0 ms/frame (fit scale 0.62) ⇒ ~16× headroom**. Rebuild-all-views is
negligible (≤ 0.03 ms/view). No gate on these numbers; keep the probe honest.
