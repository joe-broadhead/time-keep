# Repository Conventions

`time-keep` follows conventions observed in `weather-signal`, `dbt-nova`, and
`workspace-lite`.

## Convention Sources

| Source | Adopted convention |
| --- | --- |
| `weather-signal` | Rust 2024, MSRV 1.93, binary crate with committed `Cargo.lock`, CLI plus MCP server, JSON/table/CSV output, MkDocs Material docs, packaged skills, installer, strict CI, release prepare/tag/publish workflows, SBOM, provenance, and install smoke checks. |
| `dbt-nova` | Production MCP discipline, structured error posture, explicit health/readiness behavior, security-aware streamable HTTP defaults, dependency/watchlist thinking, and release PR/tag rules from `master`. |
| `workspace-lite` | Clear public contracts, API/error-code docs, mutation-safety framing, setup validation, security docs, and concise skill-oriented workflows. |

## Planned Repository Layout

```text
time-keep/
  .github/
    skills/
      time-keep/
      time-keep-calendar/
      time-keep-timers/
    workflows/
      ci.yml
      docs.yml
      release.yml
      release-prepare.yml
      release-tag.yml
  docs/
    getting-started/
    reference/
    operations/
    development/
  scripts/
    install.sh
  src/
    main.rs
    cli.rs
    mcp.rs
    app.rs
    calendar.rs
    db.rs
    models.rs
    output.rs
    util.rs
  tests/
  Cargo.toml
  Cargo.lock
  CHANGELOG.md
  CONTRIBUTING.md
  AGENTS.md
  SECURITY.md
  LICENSE
  README.md
  mkdocs.yml
  deny.toml
  rust-toolchain.toml
```

The Rust scaffold is intentionally deferred to JOE-167 so the first commit only
locks the product contract and repository policy.

## Rust Conventions

- Use Rust edition 2024.
- Set MSRV to 1.93.
- Commit `Cargo.lock` because the project is a binary crate.
- Prefer small, boring dependencies.
- Keep module boundaries behavioral: CLI parsing in `cli`, core workflows in
  `app`, MCP transport/tool routing in `mcp`, persistence in `db`, calendar/date
  behavior in `calendar`, output formatting in `output`, and shared contracts in
  `models`.
- Avoid panics in production paths.
- Use structured error codes that are stable enough for agents.

## CLI and Output Conventions

- JSON is the default public interface.
- Table output is for human inspection.
- CSV output is for spreadsheet or pipeline export.
- Logs and diagnostics go to stderr.
- Machine-readable command output stays on stdout.
- Date/time examples in docs should use absolute dates when relative phrasing
  could be ambiguous.

## MCP Conventions

- Provide stdio and streamable HTTP transports.
- Keep streamable HTTP on loopback by default.
- Reserve `/healthz` for liveness and `/readyz` for readiness.
- Warn when HTTP binds to non-loopback.
- Tool schemas, docs, tests, and skills must change together.

## Documentation Conventions

- MkDocs builds with `strict: true`.
- README gives a short product overview, install path, quickstart, CLI overview,
  MCP overview, and documentation links.
- Reference docs own detailed contracts.
- Agent skills stay concise and put reusable detail in one-level-deep
  `references/` files.

## CI and Supply-Chain Conventions

CI must include:

- `cargo fmt --check`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo test --locked --all-features`
- `mkdocs build --strict`
- `cargo deny check`
- `cargo audit`

Release automation must include:

- Version validation against `Cargo.toml`.
- Changelog validation for the release version.
- Multi-platform binary packaging.
- SHA-256 checksums.
- SBOM.
- Provenance assets and GitHub-hosted attestations when repository support is
  available.
- Install and MCP smoke checks.

## Intentional Deviations

- Unlike `weather-signal`, v0.0.0 must not use runtime network APIs.
- Unlike hosted `dbt-nova` patterns, v0.0.0 has no hosted auth or public
  multi-user service mode.
- Holiday data is explicitly bounded; docs and errors must not imply unbounded
  country/year support.
- Holiday and holiday-aware business-day behavior is local-only at runtime; do
  not introduce live holiday APIs for v0.0.0.
- Timer persistence is local SQLite only. Keep migrations in code with
  `PRAGMA user_version`, store deadlines in UTC, preserve original ISO/RFC3339
  inputs for display, default timezone-less ISO deadlines to UTC, and normalize
  tags before storage/filtering.
- The repository is private during v0.0.0 implementation even though the docs
  URL and release workflows are planned for the product.
