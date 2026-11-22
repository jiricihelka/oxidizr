# API churn running list — v0.4.0 (P4R, §P4R-V3)

Staged list of breaking-ish **Rust API** changes made by the §P4R refactor
waves. `CHANGELOG.md` does not exist yet (E-5 creates it) — every wave commit
that churns the public API appends ONE line here; E-5 folds this file into
the changelog verbatim. Frozen products (byte outputs, CLI grammar, exit
classes, OXZ codes) never appear here — they are not churn.

Format: `- <crate>: <what changed> (<wave commit>)` — newest last.

_(created at R0 2026-11-22; entries start with R2)_

- oxidizr-import: `ImportFormat` + typed `ImportFormat::sniff` constructor
  now LIVE HERE (moved out of `oxidizr-api`'s private `sniff_format`);
  `oxidizr_api::ImportFormat` stays valid as a re-export,
  `oxidizr_api::pipeline::ImportFormat` is GONE (R2, item 1)
- oxidizr-api: `Request::Diff`/`Request::Inspect` payload `json: bool` →
  typed `format: DiffFormat` / `format: InspectFormat` (new api enums,
  re-exported like the `Query` ones); `oxidizr_api::DiffFormat`/
  `InspectFormat` were the CLI's clap enums — those are renamed
  `DiffCmdFormat`/`InspectCmdFormat` in `oxidizr-cli` (grammar frozen:
  the `--format human|json` values are unchanged) (R3, item 1)
- oxidizr-api: `ApiOptions` field rework — `strict`/`keep_unknown` moved
  into a nested `parse: ParseFlags` struct (the `struct_excessive_bools`
  allow is GONE; `env`/`stdin`/`allow_net`/`themes_dir`/`color` keep
  their names); CLI/LSP/e2e/tests rewired, no behavior change (R3,
  item 2)
