# oxidizr-api

The headless pipeline behind every entry point: `execute(Request,
ApiOptions) -> Outcome` — values in, values out. One `Request` variant
per command; the `Outcome` carries the exit class, the stdout bytes and
the ordered log.

No rendering (the CLI owns that), no process-env reads (the env and stdin
seams are injected), total and thread-safe. Report schemas live in
`docs/` (diff / inspect / query).

Consumed by `oxidizr-cli`, `oxidizr-lsp`, `oxidizr-viewer` and the e2e
suite — all through this crate, never around it.
