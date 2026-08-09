# Release Policy

This project releases from `master`.

## Branches and Tags

- Default branch: `master`
- First release tag: `v0.0.0`
- Current release target: `v0.0.1`
- Tag format: `vX.Y.Z`
- Release preparation branches: `release/X.Y.Z`

Do not create or merge release branches unless the user intends to trigger the
release automation. Tag releases only after the production readiness
review/audit issue is complete.

## Release Exit Criteria

Release milestones are complete only when:

- The full CLI and MCP surface is implemented.
- Local-only deterministic behavior is documented and tested.
- SQLite timers persist with migrations and safe permissions.
- Offline holiday and business-day support is bounded and documented.
- README, MkDocs docs, agent skills, and installer are complete.
- CI, docs, release, deny, audit, SBOM, and provenance workflows pass.
- A full production readiness review and audit is complete.
- Install, binary, shell completions, MCP stdio, and streamable HTTP smoke
  checks pass from release artifacts.

## Release Flow

1. In a normal product PR, move relevant changelog entries from `Unreleased`
   into `## [X.Y.Z]` and set both Cargo metadata files to `X.Y.Z`.
2. Merge that PR to `master` after its production readiness review and all
   required checks pass.
3. Open a `release/X.Y.Z` PR through release preparation automation. This
   workflow validates metadata already on `master`; it does not change it.
4. Merge the release PR to `master` after its independent validation and
   approval gate.
5. Let release tag automation create `vX.Y.Z` from that merge.
6. Publish binaries, checksums, SBOM, provenance, and attestations through
   release automation.
7. Verify install and MCP smoke checks from published artifacts.

## Required Local Gates

Before the release tag:

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
mkdocs build --strict
cargo deny check
cargo audit
```

For every issue commit, run the relevant subset of checks plus:

```bash
codex review --uncommitted
```

Validation evidence should be recorded in the corresponding Linear issue.
