# gui-only code silently drifts past the per-commit quartet (P5R-R3j, 2026-11-25)

Symptom: the canvas split moved shell.rs bytes VERBATIM into shell/canvas.rs,
yet the `cargo xtask ci gui` leg lit up with 10 `collapsible_if` errors in the
moved code. A `git stash` + gui-clippy at HEAD proved the SAME 10 errors at
HEAD — pre-existing, not caused by the move.

Cause: the commit quartet (fmt + clippy + test + refs) never compiles
`--features gui` code (the viewer shell is gui-only by design), so clippy
drift accumulated in shell.rs unnoticed — the same lint family as
msrv-bump-clippy-wave.md, but gated behind a feature the quartet cannot see.
Any lint that only fires under a feature is invisible until some leg enables
that feature.

Fix procedure that worked: `cargo clippy --fix -p oxidizr-viewer --features
oxidizr-viewer/gui --all-targets --allow-dirty` (machine-safe here:
collapsible_if only fires when the inner `if` is the SOLE statement with no
else arm, so `if c && let p = e` short-circuits identically), `cargo fmt
--all`, re-run the gui leg to 0 errors, quartet green.

Rule: feature-gated code needs its leg in the SAME wave that touches it —
viewer/cli-gui waves put `cargo xtask ci gui` in the wave-end chain, and a
split suspected by lint runs the feature leg BEFORE its first commit.
