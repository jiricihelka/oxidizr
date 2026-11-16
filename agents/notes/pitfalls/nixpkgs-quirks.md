# nixpkgs / tooling quirks (confirmed)

- `pkgsCross.mingwW64` (win32-threads) ships **no winpthreads** (`libwinpthread.a`/
  dll missing) → linking windows-gnu self-contained std fails on `-l:libpthread.a`.
  Fix: copy `libpthread.a` from the rust-mingw component's `self-contained/` dir
  into a stub `-L` path (flake.nix `windowsGnuPthreadStub`).
- `wine64` package's loader binary is `wine`; `winetricks` is x86-only (needs
  `pkgs.i686-w64-mingw32.stdenv.cc` in `environment.systemPackages`) — not needed
  for cargo test, only for GUI-ish prefixes.
- rustup/cargo: `-C linker` cannot live in `.cargo/config.toml` ([target] is
  build-script-only) — set `CARGO_TARGET_<T>_LINKER` env instead.
- nix eval: `lib.warn`/`builtins.trace` print during `nix flake check` — trace
  noise ≠ eval error; real failures end with `error: ...` and nonzero exit.
- rustup channel dates are *build* dates; the rustc version string shows the
  previous day's commit (nightly-2026-11-13 ⇒ 8eb778d04 2026-11-12).

Last useful: never
