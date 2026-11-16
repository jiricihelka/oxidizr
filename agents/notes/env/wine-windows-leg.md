# Wine Windows leg (local Windows CI tests)

Run the whole suite against a real PE binary on Linux:

```sh
nix develop -c cargo test --workspace --target x86_64-pc-windows-gnu
# or: nix develop -c cargo xtask ci windows
```

The dev shell provides everything (flake.nix `windowsCross` + `env`):
- `buildStd = [ "x86_64-pc-windows-gnu" ]` — self-contained rust std (rust-std + rust-mingw libs);
- linker `x86_64-w64-mingw32-gcc` via per-target `CARGO_TARGET_…_LINKER` (cargo config files can't hold it: not a valid config key);
- `RUSTFLAGS` `-L <stub>` with `libpthread.a` copied from the toolchain's
  `self-contained/` — nixpkgs' win32-threads mingw has no winpthreads;
- runner `wine` — the `wine64` package's loader binary is named `wine`, NOT `wine64`.

Notes: ~2-4 min cold; leaves ~100 wine server processes — `pkill -f 'wine.*server' || wineserver -k`;
WINEPREFIX is `$HOME/wine64-drives`. It does NOT simulate the Windows *checkout*
(EOL/autocrlf, NTFS) — for EOL use `git -c core.autocrlf=true clone`; macOS stays CI-only.
Windows-native arm64 is unsupported tier-2 → CI matrix uses windows-latest x86_64 only.

Last useful: never
