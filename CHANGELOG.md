# Changelog

All notable changes to `time-keep` will be documented in this file.

The format follows Keep a Changelog, and this project uses semantic versioning
for release tags.

## [Unreleased]

### Added

- Optional `config.toml` support for a default timezone via `default_timezone`
  or `default_timezones`, plus a `TIME_KEEP_TZ` environment override. `now` and
  the `current_time` MCP tool use the configured default when no timezone is
  given. A `system`/`local` token opts in to operating-system timezone
  detection. Explicit `--tz` still wins, and the zero-config default remains
  UTC.

### Changed

- Polished public-facing README, installation, contributing, and release
  readiness docs after the v0.0.0 tag.
- Removed obsolete CLI dead-code scaffolding left over from the bootstrap plan.

## [0.0.0] - 2026-06-19

### Added

- Initial v0.0.0 product contract, repository conventions, and release policy.
- Full local-first Rust CLI for current time, timezone listing/inspection,
  timezone conversion, calendar queries, date arithmetic, date diffs,
  formatting, bounded offline holidays, business days, and SQLite timers.
- MCP stdio and streamable HTTP server with 15 local tools, health/readiness
  probes, structured tool errors, and loopback-first HTTP binding.
- SQLite timer persistence with migrations, WAL, private Unix file mode, UTC
  deadlines, normalized tags, and tag filtering.
- Offline holiday and holiday-aware business-day support with documented
  `2000..=2030` coverage and clear errors outside coverage.
- Deterministic unit and integration tests for CLI behavior, MCP protocol,
  timer persistence, timezone/calendar edge cases, holiday bounds, business-day
  behavior, output formats, and structured errors.
- MkDocs documentation, standalone agent skills, and a checksum-verifying
  release-asset installer with opt-in shell completions.
- CI, docs, release preparation, release tagging, release publishing,
  supply-chain policy, audit, SBOM, provenance assets, optional GitHub
  attestations, and release smoke workflows.
- Production readiness audit record for the v0.0.0 release gate.
