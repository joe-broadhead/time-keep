# AGENTS.md

Guidance for coding agents working in this repository.

## Project Overview

`time-keep` is a Rust CLI and MCP server for reliable agent time context. It is
local-first: current time, timezone operations, calendar math, business days,
holiday lookups, and timers must work without hosted auth, cloud sync, network
time APIs, or live holiday APIs.

Core areas:

- `src/main.rs` - binary entrypoint, tracing, and command dispatch.
- `src/cli.rs` - Clap command, option, and parser definitions.
- `src/app.rs` - application workflows and shared command handlers.
- `src/mcp.rs` - MCP stdio and streamable HTTP transports and tool routing.
- `src/calendar.rs` - calendar queries, date arithmetic, and business-day math.
- `src/db.rs` - SQLite timer persistence, migrations, WAL setup, and tags.
- `src/models.rs` - shared data contracts and response envelopes.
- `src/output.rs` - JSON, table, and CSV rendering.
- `src/util.rs` - XDG paths, parsers, and timezone validation helpers.
- `.github/skills/` - packaged agent skills.
- `.github/workflows/` - CI, docs, release prepare/tag/publish automation.
- `docs/` - MkDocs Material documentation site.

## High-Signal Commands

Run focused checks while developing, then run release-grade checks before
handoff.

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
mkdocs build --strict
cargo deny check
cargo audit
```

Before committing each Linear issue, run the repo-relevant validation commands
and the Codex autoreview gate:

```bash
codex review --uncommitted
```

When editing workflow files, run `actionlint` if it is available.

## Development Rules

- Keep each commit scoped to one Linear issue.
- Preserve the public CLI and MCP contracts unless the Linear issue
  explicitly changes the contract.
- JSON is the default and source-of-truth output contract. Table and CSV output
  should be useful, but must not redefine behavior.
- Use UTC when no timezone is explicitly supplied unless a future config default
  is intentionally designed and documented.
- Accept IANA timezone names only. Do not silently fall back to local system
  time for invalid input.
- Keep timers local in SQLite. Do not introduce hosted state or cloud sync.
- Keep holiday behavior offline and bounded. Document coverage and fail clearly
  outside supported ranges.
- Add or update tests for behavior changes.
- Prefer explicit error propagation over panics or silent fallback.
- Do not add `.expect()` or `.unwrap()` in production paths unless the invariant
  is strong and the message is useful.
- Keep source and docs ASCII unless external names or data require otherwise.
- Do not commit generated build artifacts, caches, local config, or `site/`.

## Release Rules

- The default branch is `master`.
- The first release tag is `v0.0.0`.
- Release tags use `vX.Y.Z`.
- Do not use `release/*` branch names unless the user intends to trigger the
  release automation after merge.
- Tag releases only after the production readiness review is complete.
- For user-visible behavior, update `CHANGELOG.md`.
- Release assets must include binaries, checksums, SBOM, and provenance.

## Security and Privacy

- Never log or commit tokens, credentials, private keys, or personal timer data.
- Keep examples and tests synthetic.
- Runtime behavior must not call network time APIs or live holiday APIs.
- Streamable HTTP should bind to loopback by default and warn on non-loopback
  binds.
- Treat local SQLite files and config as private user data.

## PR Checklist for Agents

- Run the narrowest meaningful tests for the touched code.
- Run formatting and lint checks when code changed.
- Run docs build when docs changed.
- Run `cargo deny check` and `cargo audit` when dependencies or release policy
  changed.
- Run `codex review --uncommitted` before committing.
- Include validation evidence in Linear before marking an issue done.
