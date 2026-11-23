# cargo-dist panics `couldn't find the release!?` when installers are configured

**Symptom.** `dist plan` / `dist build` abort with
`× dist panicked. at cargo-dist/src/tasks.rs:2055:14 — couldn't find the release!?`
(line 2023 in 0.31–0.33) as soon as `installers` is set in the dist config.
Reproduced 2026-11-23 on a **vanilla `cargo new` project**: official GitHub
binaries 0.31.0/0.32.0/0.33.1/0.34.0 AND the nixpkgs binary; tagged and
untagged; config in `[workspace.metadata.dist]` (Cargo.toml, v0) or in
`dist-workspace.toml` without `ci`/`hosting`. Without `installers`, plan
succeeds (archives + sha256 only). No upstream issue found; the `.expect` is
still on `main` (tasks.rs:2055/2146/2305/2386).

**Cause.** `add_shell_installer` (and friends) do
`manifest.release_by_name(&release.app_name).expect(...)` but the manifest's
releases are only populated by `add_releases_to_manifest` (tasks.rs:3048) —
after the per-release installer loop. GitHub hosting set up first seeds the
manifest, so the lookup only succeeds when hosting is actually configured.

**Fix.** Use the v1 `dist-workspace.toml` with `ci = "github"` and
`hosting = "github"` (plus a resolvable `repository` URL); the panic
disappears and `dist generate --mode ci` writes the workflow. Notes:
`rust-toolchain-version` is deprecated in 0.34 (it reads
`rust-toolchain.toml`); v1 GitHub CI has NO custom-job hook (only
runners/permissions/build-setup/action-commits), so post-release smoke steps
need their own workflow — keep `release.yml` regenerable.
