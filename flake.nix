{
  description = "oxidizr — a Rust alternative to Structurizr with full DSL/JSON compatibility";

  # Development environment only (AGENTS.md §Dev environment): CI and end users build
  # with plain cargo and must not need nix. This flake pins what P1+ development needs:
  # the nightly Rust toolchain, the pinned `structurizr` reference binary (Category B
  # oracle), and the optional helper binaries the plan discovers on PATH (graphviz for
  # the P3 `dot-backend` feature, plantuml for the P2 export render checks).

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    # Nightly Rust (AGENTS.md §Toolchain), pinned through flake.lock. `rust-toolchain.toml`
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
      # library 6.2.4 = the pinned monorepo commit `ad34036` (AGENTS.md §Source of
      # truth). Goldens are generated only via `cargo xtask oracle`; record this version
      # in testdata/README.md when it is created (M0). Requires Java 21 (docs + pom
      # `maven.compiler.release`).
      structurizrVersion = "2026.10.26";

      # Windows CI runs natively (AGENTS.md §Portability), so no Windows system here.
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
            }
          );
        in
        {
          default = pkgs.mkShell {
            name = "oxidizr-dev";

            packages = [
              nightly

              # The pinned reference oracle on PATH ($OXIDZR_STRUCTURIZR_BIN → PATH
              # discovery, AGENTS.md §Testing) — plus a JDK to poke at wars by hand.
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
            ];

            # Explicit oracle pin; users may override, tests fall back to PATH without it.
            OXIDZR_STRUCTURIZR_BIN = "${self.packages.${system}.structurizr}/bin/structurizr";

            shellHook = ''
              echo "oxidizr dev shell: $(rustc --version) · oracle: structurizr ${structurizrVersion} (\$OXIDZR_STRUCTURIZR_BIN)"
            '';
          };
        });
    };
}
