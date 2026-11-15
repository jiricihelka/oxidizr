{
  description = "oxidizr — a Rust alternative to Structurizr with full DSL/JSON compatibility";

  # Development environment only (agents/plan/WORKFLOW.md §Dev environment): CI and end users build
  # with plain cargo and must not need nix. This flake pins what P1+ development needs:
  # the nightly Rust toolchain, the pinned `structurizr` reference binary (Category B
  # oracle), and the optional helper binaries the plan discovers on PATH (graphviz for
  # the P3 `dot-backend` feature, plantuml for the P2 export render checks).

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    # Nightly Rust (agents/plan/WORKFLOW.md §Dev environment), pinned through flake.lock. `rust-toolchain.toml`
    # (M0) is the authoritative pin for plain-cargo users; update both in step and
    # re-check for stabilization per NIGHTLY.md at each milestone.
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    { self, nixpkgs, rust-overlay, ... }:
    let
      # The reference oracle: the official unified application binary (Spring Boot war,
      # `java -jar structurizr.war export -format json ...`, see
      # https://docs.structurizr.com/binaries). Application release 2026.10.26 bundles
      # library 6.2.4 = the pinned monorepo commit `ad34036` (agents/plan/COMPAT.md §Source of
      # truth). Goldens are generated only via `cargo xtask oracle`; record this version
      # in testdata/README.md when it is created (M0). Requires Java 21 (docs + pom
      # `maven.compiler.release`).
      structurizrVersion = "2026.10.26";

      # Windows CI remains the authority (agents/plan/WORKFLOW.md §Code quality, native runners), but
      # on Linux the flake also provides an approximating local leg: windows-gnu
      # cross-compile + Wine test execution (see the dev shell). macOS only ever runs
      # in CI (no darwin cross from Linux).
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system nixpkgs.legacyPackages.${system});
    in
    {
      # `packages.oxidizr` (the cargo workspace) joins this set at M0.

      packages = forAllSystems (_system: pkgs: {
        structurizr = pkgs.stdenvNoCC.mkDerivation {
          pname = "structurizr";
          version = structurizrVersion;

          src = pkgs.fetchurl {
            url = "https://download.structurizr.com/structurizr-${structurizrVersion}.war";
            hash = "sha256-SVRDUX3CGGCHwywuttdiqaJlyN6r5QmwctSb6o0wSaQ=";
          };

          dontUnpack = true;
          nativeBuildInputs = [ pkgs.makeWrapper ];

          installPhase = ''
            runHook preInstall
            mkdir -p $out/share/java $out/bin
            install -m644 $src $out/share/java/structurizr.war
            makeWrapper ${pkgs.jdk21}/bin/java $out/bin/structurizr \
              --add-flags "-jar $out/share/java/structurizr.war"
            runHook postInstall
          '';

          meta = {
            description = "Structurizr unified application binary (oxidizr's Category B reference oracle)";
            homepage = "https://github.com/structurizr/structurizr";
            changelog = "https://github.com/structurizr/structurizr/releases/tag/v${structurizrVersion}";
            license = pkgs.lib.licenses.asl20;
            sourceProvenance = [ pkgs.lib.sourceTypes.binaryNativeCode ];
            # the war embeds the full monorepo (Apache-2.0) at the pinned release
            mainProgram = "structurizr";
          };
        };
      });

      devShells = forAllSystems (system: pkgs:
        let
          # Nightly Rust (Builder interface of rust-overlay; README: only
          # overlays.* are stabilized outputs, lib.mkRustBin is the flake-native
          # entry). The override rides INSIDE selectLatestNightlyWith so the
          # selection skips nights missing any of our components. Components:
          # rust-src (clippy), llvm-tools (cargo-llvm-cov), rust-analyzer (agents).
          # Target: x86_64-pc-windows-gnu for the local Wine leg (see `windowsCross`).
          rust-bin = rust-overlay.lib.mkRustBin { } pkgs;
          nightly = rust-bin.selectLatestNightlyWith (
            toolchain:
            toolchain.minimal.override {
              extensions = [
                "clippy"
                "rustfmt"
                "rust-src"
                "llvm-tools"
                "rust-analyzer"
              ];
              targets = [ "x86_64-pc-windows-gnu" ];
              # Target-scoped component (host-scoped `extensions` rejects it): ships
              # the mingw import libs (-l:libpthread.a etc.) the windows-gnu link needs.
              targetExtensions = [ "rust-mingw" ];
            }
          );

          # Local Windows test loop (Linux hosts only): mingw-w64 linker + Wine
          # runtime. Usage: `cargo test --workspace --target x86_64-pc-windows-gnu`
          # — cargo runs the test binaries through $CARGO_TARGET_..._RUNNER.
          # Approximates the windows-latest CI leg (real PE, Wine paths), NOT the
          # Windows checkout (EOL/autocrlf: reproduce via `git -c core.autocrlf=true
          # clone` instead) and not CI's authority.
          windowsCross = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
            pkgs.pkgsCross.mingwW64.stdenv.cc # x86_64-w64-mingw32-gcc
            pkgs.wine64
          ];

          # nixpkgs' win32-threads mingw omits winpthreads (libpthread.a), which the
          # windows-gnu link of std still references; Rust's own `rust-mingw`
          # component ships the stub, so expose just that archive on the link path
          # (a whole-dir -L would shadow the nixpkgs mingw libs).
          windowsGnuPthreadStub = pkgs.runCommand "windows-gnu-pthread-stub" { } ''
            mkdir -p "$out"
            ln -s "${nightly}"/lib/rustlib/x86_64-pc-windows-gnu/lib/self-contained/libpthread.a "$out/libpthread.a"
          '';
        in
        {
          default = pkgs.mkShell {
            name = "oxidizr-dev";

            packages = [
              nightly

              # The pinned reference oracle on PATH ($OXIDZR_STRUCTURIZR_BIN → PATH
              # discovery, agents/plan/TESTING.md) — plus a JDK to poke at wars by hand.
              self.packages.${system}.structurizr
              pkgs.jdk21

              # Optional-helper discovery paths (never required for Category A builds).
              pkgs.graphviz # `dot` — P3 dot-backend dev; plantuml also needs it
              pkgs.plantuml # P2 render checks ($OXIDZR_PLANTUML_BIN → PATH)

              # Dev-only tooling from the testing policy (report-only coverage, fuzzing).
              pkgs.cargo-llvm-cov
              pkgs.cargo-fuzz

              pkgs.git
              pkgs.pkg-config
            ]
            # local windows-gnu test leg — mingw-w64 + wine64 (Linux only; nested
            # lists in `packages` are deprecated nixpkgs 26.05, so splice with ++)
            ++ windowsCross;

            # Explicit oracle pin; users may override, tests fall back to PATH without it.
            OXIDZR_STRUCTURIZR_BIN = "${self.packages.${system}.structurizr}/bin/structurizr";

            # Windows leg under Wine (Linux host): cargo executes windows-gnu test
            # binaries via Wine (the `wine64` package's loader is named `wine`);
            # keep its console quiet; satisfy -l:libpthread.a via the stub -L.
            CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER = "wine";
            CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS = "-L native=${windowsGnuPthreadStub}";
            WINEDEBUG = "-all";

            shellHook = ''
              echo "oxidizr dev shell: $(rustc --version) · oracle: structurizr ${structurizrVersion} (\$OXIDZR_STRUCTURIZR_BIN)"
              # Wine needs a writable prefix + runtime dir; first use self-initializes.
              export WINEPREFIX="''${XDG_CACHE_HOME:-''${HOME:-/tmp}/.cache}/oxidizr/wine64"
              mkdir -p "$WINEPREFIX"
              if [ -z "''${XDG_RUNTIME_DIR:-}" ]; then
                export XDG_RUNTIME_DIR="/tmp/oxidizr-wine-runtime-$(id -u)"
                mkdir -pm 0700 "$XDG_RUNTIME_DIR"
              fi
            '';
          };
        });
    };
}
