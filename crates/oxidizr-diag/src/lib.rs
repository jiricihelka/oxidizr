//! `oxidizr-diag` — shared diagnostic plumbing (P1-D2):
//!
//! * [`Span`]` { file, start: (line, col), end }` — 1-based, UTF-8 char counts.
//! * [`Severity`] — `Error` / `Warning` / `Info`.
//! * [`Code`] — `OXZ-nnnn` newtype; ranges: 1xxx lexer, 2xxx parser, 3xxx model &
//!   validation, 4xxx JSON, 5xxx I/O & CLI policy, 9xxx internal. Every code is
//!   documented in `docs/diagnostics.md` and stable post-1.0.
//! * Collectors: parsers/validators return `Vec<Diagnostic>` (cap 100 + abort note).
//!
//! No `miette` here: the CLI converts at its boundary (P1-D2), libraries stay
//! dependency-free.
//!
//! [`Span`]: https://github.com/jiricihelka/oxidizr (type lands in M1)
