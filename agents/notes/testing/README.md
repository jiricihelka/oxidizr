# testing — test architecture

| note | one-liner |
|---|---|
| [byte-exact-corpus.md](byte-exact-corpus.md) | testdata/ is a byte contract; `.gitattributes` pins EOL repo-wide |
| [burn-in-register.md](burn-in-register.md) | m4_fixtures.rs KNOWN_ERRORS pattern for deferred features |
| [local-ci-mirrors.md](local-ci-mirrors.md) | `cargo xtask ci|bench|cov` — argv is literally ci.yml's |
| [ast-grep-search.md](ast-grep-search.md) | structural code search recipes (`$X.unwrap()` etc.) |
| [oracle-app-harness.md](oracle-app-harness.md) | export injects inspection props + default views; goldening limits |
| [cli-e2e-pitfalls.md](cli-e2e-pitfalls.md) | subprocess CLI tests: piped stdio, random UUID identifiers, known-vs-legacy fields |
| [e2e-suite-operations.md](e2e-suite-operations.md) | e2e corpus tiers, bless/mutate workflow + the bless-REVIEW rule (exit-class/counts smell), bench feature |
| [fuzz-p4t-finds.md](fuzz-p4t-finds.md) | P4T fuzz battery: two real engine finds (BOM fmt, zero-width matcher), bounded-surface rule, fuzz-workspace drift gate |
