# oxidizr-json

The JSON contract: readers and writers for Structurizr workspace JSON that
reproduce the official format exactly — alphabetical keys, empty-value
elision, sorted set-backed arrays, hash-bucket property ordering,
computed view names.

Both directions are hand-rolled because the standard derive route cannot
express those ordering rules; byte-for-byte output is the test bar
(goldens compared in the e2e suite).

Key entries: `to_vec` / `from_slice` (workspaces), `theme_*` (theme
files), `roundtrip_keep_unknown` (the `--keep-unknown` superset leg),
`javahash` (the property-order hash).

Consumed by `oxidizr-api`, `oxidizr-export`.
