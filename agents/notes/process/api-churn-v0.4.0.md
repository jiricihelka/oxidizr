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
