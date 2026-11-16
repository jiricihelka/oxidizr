# env — dev environment setup

| note | one-liner |
|---|---|
| [wine-windows-leg.md](wine-windows-leg.md) | run the Windows CI leg locally: windows-gnu cross + Wine |
| [reference-oracle.md](reference-oracle.md) | structurizr oracle: pins, invocation contract, where the jar is |
| [nix-flake.md](nix-flake.md) | what `nix develop` provides; the nightly double-pin rule |
| [toolbox.md](toolbox.md) | gates + verification-loop tools on PATH, when/how; what's absent |
| [triage-toolkit.md](triage-toolkit.md) | inspection/search tools (jq, ast-grep, …) + env knobs |
| [fuzz-host-toolchain.md](fuzz-host-toolchain.md) | `cargo xtask fuzz` strips the shell's mingw `CC/CXX/AR/RANLIB` (cc-rs leak) |
