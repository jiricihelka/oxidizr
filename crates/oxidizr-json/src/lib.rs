//! `oxidizr-json` — the JSON contract (P1-D5): serde derives over `oxidizr-core`
//! types, `rename_all = "camelCase"` where names diverge, POJO fields **declared
//! alphabetically** because `serde_json` streams struct fields in declaration order —
//! that declaration order *is* the golden's POJO key order. Free-form `properties`
//! maps are the exception: the reference serializes them in Java `HashMap` order
//! (`PHASE_1` V-11), settled at M3 (replicate or canonicalize in the comparator).
//!
//! Writer omissions mirror the reference per-field (`skip_serializing_if`), pinned by
//! `plan/build/WRITE-CATALOG.md` (P1-D6) — never guessed. Output is Jackson-style
//! pretty: 2-space indent and `"key" : value` spacing (custom `serde_json` Formatter;
//! see plan/PROGRESS.md M0 note — `to_string_pretty` alone does NOT byte-match).
//!
//! Reader leniency: string-or-number ids, empty-as-none, numeric tolerance,
//! `enterpriseContextViews` alias, unknown keys ignored (captured only under
//! `--keep-unknown`, D3). Dates: UTC `yyyy-MM-dd'T'HH:mm:ss'Z'` on write, tolerant on
//! read.
// P1-M0 skeleton: manifest declares the agreed P1-D9 set before the code that uses
// it (M3+). Remove this allow when the derives land.
#![allow(unused_crate_dependencies)]
