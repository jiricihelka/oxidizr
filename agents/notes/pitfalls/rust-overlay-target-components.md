# rust-overlay: target components go in `targetExtensions`

`rust-mingw` (mingw import libs: `-l:libpthread.a` etc. for windows-gnu links) is
**target-scoped**. Putting it in `minimal.override { extensions = [ "rust-mingw" ]; }`
makes that candidate fail to evaluate — and `selectLatestNightlyWith` then **silently
falls back ~100 nights** to the last nightly where the host-scoped component existed
(saw 2026-08-07 while pinned 2026-11-13). Symptom: `rustc -Vv` ≠ the pin, no error.

Correct form (flake.nix):

```nix
toolchain.minimal.override {
  extensions = [ "clippy" "rustfmt" "rust-src" "llvm-tools" "rust-analyzer" ];
  targets = [ "x86_64-pc-windows-gnu" ];
  targetExtensions = [ "rust-mingw" ];   # ← key = target via targets
}
```

The flake's `pinMatch` guard exists precisely to catch this class of silent drift.
