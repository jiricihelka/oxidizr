# NixOS viewer start: winit `NoWaylandLib` (fixed 2026-11-20)

Symptom: `Error: WinitEventLoop(Os(… WaylandError(Connection(NoWaylandLib))))` on
a Wayland session (Hyprland here). Not a code bug: winit/x11-dl/xkbcommon-dl/ash
**dlopen** `libwayland-client/-cursor`, `libxkbcommon`, `libvulkan`,
`libX11/libXcursor/libXi/libXrandr` at RUNTIME — link-time sees none of them
(`ci gui` compiles fine without) and NixOS has no system loader dir to find
them in (`ldconfig -p | grep libwayland` → 0 hits).

Fix (flake `guiRuntime`): `shellHook` exports `LD_LIBRARY_PATH` built with
`pkgs.lib.makeLibraryPath`. Membership in mkShell `packages`/`buildInputs`
alone bakes in NOTHING — no RUNPATH (nothing links against them) and no
reliable env — the explicit export is what works. NO rebuild needed: the env
fixes an already-built binary. GPU drivers ride the system `hardware.graphics`
(`/run/opengl-driver`), not the flake. `WINIT_UNIX_BACKEND=x11` is NOT a fix
(libX11 is just as absent); it only selects the fallback once the env exists.

Verify: `timeout 5 nix develop -c ./target/debug/oxidizr-viewer
testdata/dsl/big-bank-plc.dsl` → exit 124 (alive, window shown), no `Error:` line.
Outside the shell ad hoc: prepend `LD_LIBRARY_PATH` with
`$(nix build --no-link --print-out-paths nixpkgs#wayland nixpkgs#vulkan-loader
nixpkgs#libxkbcommon …)/lib` — joined by `:` per path.
