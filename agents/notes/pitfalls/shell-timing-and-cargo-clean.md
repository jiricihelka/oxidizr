# Wall-clock timing + `cargo clean --workspace` on this box

Paid 2026-11-18/19 (P2R §D4 V-R2 ratchet measurement; two invalid runs).

- `/usr/bin/time` does NOT exist here (Nix bash) and the failure is SILENT in a
  `{ …; } 2> out.txt` harness: the stub error lands in the capture file, the shell
  still exits 0, and the "measurement" reads like a legit zero-fire result. Time with
  shell-internal `S=$(date +%s%N); …; E=$(date +%s%N)` instead.
- `cargo clean --workspace` keeps the DEPENDENCY cache (proven: afterwards clippy
  rebuilds exactly the 51 workspace units in ~2.3 s, no deps). Its size varies with
  what accumulated: 21810 files/7.6 GiB first (all historical debug+release/bench
  legs), then 570 files/95.6 MiB (the debug rebuild only). Fine for lint A/B deltas;
  never call it "fully cold".
- A clippy-only rebuild of this workspace IS ~2.3 s (51 units, no codegen) — do not
  distrust a 2–3 s figure; distrust a 2 s one only if a codegen step should be inside.
- Prove the measurement rebuilt by counting `Running` lines in `-v` output, and run
  n≥3 per arm: cold-arm medians 2365 vs 2466 ms (≈ +4 %) overlap at n=1, decided at n=3.

Last useful: 2026-11-19
