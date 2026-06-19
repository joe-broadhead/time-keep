# Release

time-keep releases from `master`.

## Prepare

Before preparing a release:

1. Move user-visible changes from `CHANGELOG.md` `Unreleased` into the target
   version section.
2. Ensure `Cargo.toml` `version` matches the target version.
3. Run the local quality gates.

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
mkdocs build --strict
cargo deny check
cargo audit
```

## Release Automation

Release automation lives in `.github/workflows/`:

- `ci.yml` runs format, clippy, tests, docs, `cargo deny`, and `cargo audit`.
- `docs.yml` builds MkDocs and publishes GitHub Pages from `master`.
- `release-prepare.yml` creates a `release/X.Y.Z` PR after metadata checks.
- `release-tag.yml` tags merged release or hotfix PRs with `vX.Y.Z`.
- `release.yml` validates the tag, builds platform archives, publishes
  SHA-256 checksums, generates SBOM and provenance JSON assets, publishes
  GitHub-hosted provenance attestations when repository support is available,
  smokes the installer from the generated Linux asset, and uploads release
  assets only after the full platform matrix succeeds.

`release-prepare.yml` requires a `RELEASE_PR_TOKEN` secret backed by a PAT or
GitHub App token with `contents:write` and `pull-requests:write` so the release
branch push and PR creation trigger the required CI checks. `release-tag.yml`
requires `RELEASE_TAG_TOKEN` with `contents:write` so pushing `vX.Y.Z` triggers
the release workflow.

The documented asset set is four files per platform:

```text
time-keep-linux-x86_64.tar.gz
time-keep-linux-x86_64.sha256
time-keep-linux-x86_64.sbom.spdx.json
time-keep-linux-x86_64.provenance.json
time-keep-macos-x86_64.tar.gz
time-keep-macos-x86_64.sha256
time-keep-macos-x86_64.sbom.spdx.json
time-keep-macos-x86_64.provenance.json
time-keep-macos-arm64.tar.gz
time-keep-macos-arm64.sha256
time-keep-macos-arm64.sbom.spdx.json
time-keep-macos-arm64.provenance.json
time-keep-windows-x86_64.tar.gz
time-keep-windows-x86_64.sha256
time-keep-windows-x86_64.sbom.spdx.json
time-keep-windows-x86_64.provenance.json
```

## Release Status

`v0.0.0` was cut from `master` after JOE-177, the production readiness review
and audit, was complete. JOE-174 records the final release execution and
published artifact smoke tests.
