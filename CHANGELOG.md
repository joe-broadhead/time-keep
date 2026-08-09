# Changelog

All notable changes to `time-keep` will be documented in this file.

The format follows Keep a Changelog, and this project uses semantic versioning
for release tags.

## [Unreleased]

## [0.0.1] - 2026-08-09

### Added

- Optional `config.toml` support for a default timezone via `default_timezone`
  or `default_timezones`, plus a `TIME_KEEP_TZ` environment override. `now` and
  the `current_time` MCP tool use the configured default when no timezone is
  given. A `system`/`local` token opts in to operating-system timezone
  detection across Linux, macOS, Windows, and other supported targets
  (validated against the IANA database, with POSIX path-form `TZ` and
  `posix/`/`right/` tzdata variants handled). Unmappable POSIX `TZ` rules fail
  explicitly instead of silently falling back to a different timezone.
  Explicit `--tz` still wins, an explicit empty MCP `timezones` list still
  means UTC, an explicitly empty config list wins over the singular setting,
  and the zero-config default remains UTC. Only the commands that use the
  default read the config file, unknown config keys warn without corrupting
  structured errors, the env override applies even when the config file is
  invalid, and the MCP server re-resolves per call (picking up config and OS
  timezone changes) instead of failing to start on a bad default.

### Changed

- Updated the packaged agent skill to distinguish the zero-config UTC fallback
  from configured timezone defaults.
- Added release-platform PR builds and system-timezone smoke coverage for Linux,
  Apple Silicon and Intel macOS, and Windows.
- Refreshed public-facing README, installation, contributing, and release
  readiness docs for the v0.0.1 release.
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
