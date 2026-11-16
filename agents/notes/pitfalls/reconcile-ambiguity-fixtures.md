# reconcile ambiguity fixtures (OXZ-3002)

Why: hand-building a layout JSON with duplicate names to trigger an ambiguous
remap FAILS before `reconcile` runs — two gates reject duplicate element names,
one per input format.

Facts:
- DSL parse: "A top-level element named 'X' already exists." — enforced in
  core (`CoreError::DuplicateTopLevelName`, `builder.rs`), so BOTH parse paths
  and every kind-pair check it; groups do not scope it.
- JSON read: `from_slice` hydrate validation: "A person or software system
  named \"X\" already exists." → a duplicate-name layout JSON never loads.
- Canonical names are kind-prefixed (`Person://Bob` ≠ `SoftwareSystem://Bob`),
  so a person and a system named alike never contest — dropping one is an
  unambiguous deletion.
- The fixture that DOES contest: the edited element takes the identifier of
  one old element and the NAME of another — `x = "Foo"` + `y = "Bar"` edited
  into `x = "Bar"`: the identifier match and the canonical-name match resolve
  to ONE new id → OXZ-3002, both claims revoked (first claim is un-grafted
  too), geometry stays 0/0, JSON still emitted, exit 1.

Verify: `cargo test -p oxidizr-cli --lib reconcile` (contested_claim_…) and
`cargo test -p oxidizr-cli --test cli reconcile_ambiguous_contest…`.

Last useful: 2026-11-16
