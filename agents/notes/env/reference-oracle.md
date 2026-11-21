# structurizr reference oracle

Pinned pair (keep in lockstep, see flake.nix + `xtask` REFERENCE_* consts):
app release **2026.10.26** = library 6.2.4 = monorepo commit **ad34036**.
Requires Java 21. Flake packages the war as `pkgs.structurizr` (makeWrapper
`java -jar structurizr.war`); nix-dev builds also expose
`OXIDZR_STRUCTURIZR_JAR=~/.local/share/structurizr-cli/structurizr-cli-<ver>.jar`.

Invocation contract (verified against 2026.10.26 — the old CLI's single-dash
`-format` form is REJECTED by the unified binary):

```
structurizr export --workspace F --format json --output DIR   # DIR gets <basename>.json
```

Rules: the binary is SLOW (JVM start ~1 s) — Category B only, default-OFF
`reference` feature; goldens committed, per-PR CI never calls it. Discovery:
`$OXIDZR_STRUCTURIZR_BIN`, else `structurizr` on PATH; missing → skip with loud
summary (except the dedicated CI job, where skip = failure).
Drivers: `cargo xtask oracle gen|verify` (normalizes the reference's 2-space
pretty-print + scope-order quirks — see `xtask::normalize_reference_output`).

Last useful: 2026-11-20 — `oracle verify` green: 4 json + **4 sidecar** (M5-1
saved-sidecar A/B legs: identity re-export byte-equal; edited layered
geometry round-trips the loader) + 90 diagram + 6 theme golden sets.

Manual A/B legs (§P4-M2 close, 2026-11-21, `TZ=UTC`): DSL→JSON vs
`export --workspace X.dsl`, compare via `jq -S` after masking TWO
classes — `structurizr.dsl.identifier` UUIDs are RANDOM per-parse on
BOTH sides (mask to a constant), and the CLI injects
`structurizr.inspection.*` counters into TOP-LEVEL `properties` at
export (strip; delete the key when it empties — oxidizr never writes
it). Read gate: the reference re-exports our JSON with trailing-EMPTY
tags dropped (its `setTags` = `split(",")` on import — the reference's
own asymmetry, mask it). These legs caught the Java empty-input `split`
special case (`"".split(",")` → `[""]`, JVM-proven `java T.java`): an
empty tags token stores an EMPTY tag ⇒ `"Element,Deployment Node,"`.
