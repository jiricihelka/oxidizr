# ast-grep — structural code search (dev shell)

`grep` matches text; `ast-grep` matches parsed syntax — comment/string noise and
multi-line constructs excluded. Measured here (2026-11-15): `.unwrap()` → grep
207 hits (12 comment prose) vs ast-grep 191 real sites. Binary `ast-grep`
(alias `sg`); patterns use `$X` capture, `$$$` rest.

## Invocation

    ast-grep -p '<pattern>' -l rust crates                     # matches
    ast-grep -p '<pattern>' -l rust crates --json=compact | jq # count; .[].metaVariables filters captures
    ast-grep -p '<pat>' -r '<replacement>' -U crates           # rewrite (-U = apply!)

## Verified on this workspace

| want | pattern (count) |
|---|---|
| unwrap call sites (charter audit) | `$X.unwrap()` (191) |
| bare error-macro sites | `bail!($M)` (9) |
| fns with ≥1 param | `fn $F($A, $$$P) {$$$B}` (51) |
| any derive, multi-line safe | `#[derive($$$A)]` (124) |

Not for §doc-refs or JSON goldens (lexical is right there); no grammar for
`.dsl`. rust-analyzer/ctags deliberately NOT in the shell — one-shot queries
don't amortise a cold index (decision 2026-11-15).
