# Product Contract

This document is the implementation contract for `time-keep` v0.0.0.

## Identity

| Field | Value |
| --- | --- |
| Name | `time-keep` |
| Repository | `joe-broadhead/time-keep` |
| Visibility | Private during v0.0.0 implementation |
| License | MIT |
| Language | Rust |
| Edition | 2024 |
| MSRV | 1.93 |
| Default branch | `master` |
| First tag | `v0.0.0` |
| Docs URL | `https://joe-broadhead.github.io/time-keep/` |

## Outcome

Ship a local-first Rust CLI and MCP server that gives agents reliable time
context:

- Current time in one or more IANA timezones.
- Timezone listing, inspection, conversion, UTC offsets, DST status, and next
  transition metadata when available.
- Calendar queries, date arithmetic, date diffs, and date formatting.
- Offline holiday lookups with explicit bounded coverage.
- Business-day calculations that can use weekends only or country holidays.
- Local SQLite timers with durable persistence, migrations, and tag filtering.
- MCP stdio and streamable HTTP transports for agent clients.

## Operating Principles

- One Linear project tracks the product.
- Milestones are release slices.
- Issues are concrete implementation units with acceptance criteria and
  validation commands.
- Commits are scoped one issue at a time.
- Release tags are cut only after the production readiness review/audit is
  complete.

## Public CLI Surface

Global output flags:

- `--output json|table|csv`
- `--table`
- `--config <path>`
- `--data-dir <path>`

Command families:

- `now`
- `tz info`
- `tz list`
- `convert`
- `format`
- `calc add`
- `calc subtract`
- `calc diff`
- `calendar`
- `biz between`
- `biz next`
- `biz prev`
- `holiday check`
- `holiday list`
- `timer set`
- `timer get`
- `timer list`
- `timer delete`
- `timer check`
- `config path`
- `server start`
- `completions`

JSON is the default output mode and the source-of-truth contract.

## MCP Surface

The v0.0.0 MCP server exposes 15 tools:

| Tool | Purpose |
| --- | --- |
| `current_time` | Current datetime in UTC plus requested IANA zones. |
| `list_timezones` | List IANA timezone identifiers with optional region filtering. |
| `timezone_info` | Describe offset, DST state, abbreviation, and next transition. |
| `timer_set` | Create or update a named timer. |
| `timer_get` | Read one timer and computed pending/overdue status. |
| `timer_list` | List timers with optional tag filtering. |
| `timer_delete` | Delete a named timer and associated tags. |
| `timer_check` | Return all currently overdue timers. |
| `calendar_query` | Return week, day-of-year, days-in-month, leap year, and quarter. |
| `holidays` | Check a holiday or list holidays for a country/year. |
| `business_days` | Count business days or find next/previous business day. |
| `date_arithmetic` | Add or subtract date/time units. |
| `date_diff` | Compute differences between two dates. |
| `date_format` | Parse and format dates into supported formats. |
| `convert_timezone` | Convert a datetime between IANA timezones. |

Transports:

- stdio for local MCP clients.
- streamable HTTP at loopback by default, with JSON-RPC `POST /mcp`,
  `/healthz`, and `/readyz`.
- streamable HTTP is stateless in v0.0.0; streaming `GET /mcp` returns `405`
  unless a future issue intentionally adds server-sent event streaming.
- non-loopback HTTP binds must warn clearly.
- graceful shutdown must drain active work.

## Data, Time, and Calendar Rules

- UTC is the default timezone when no timezone is supplied.
- IANA timezone names are required for timezone-aware operations.
- Invalid timezone names return structured parameter errors.
- Date arithmetic must document month-end and leap-year behavior.
- Holidays are offline and bounded to `2000..=2030` unless future work expands
  and proves wider coverage.
- Holiday and holiday-aware business-day operations must not make runtime
  network calls.
- Requests outside holiday coverage fail clearly with supported coverage in the
  error details.
- Business-day `between` uses Excel-style inclusive endpoints.
- Business-day `between` is weekend-only unless holiday skipping is explicitly
  enabled with a country.
- Business-day `next` and `prev` use strict after/before semantics.

## Timer Persistence

Timers are the only persistent runtime state in v0.0.0.

- Default database path: `XDG_DATA_HOME/time-keep/timers.db`.
- Fallback database path: `~/.local/share/time-keep/timers.db`.
- `TIME_KEEP_DATA_DIR` and `--data-dir` can redirect the data directory.
- SQLite must use WAL, `busy_timeout=5000`, and `foreign_keys=ON`.
- Migrations use `PRAGMA user_version`.
- Unix files should be private where feasible.
- Deadlines are stored in UTC while preserving original ISO/RFC3339 input and
  resolved offset for display. Timezone-less ISO datetimes default to UTC.
- Tags are normalized to lowercase, deduplicated, sorted, and stored in a
  queryable table.

## Config Paths

- Default config path: `XDG_CONFIG_HOME/time-keep/config.toml`.
- Fallback config path: `~/.config/time-keep/config.toml`.
- Config remains TOML and human-readable.
- Config examples must not include credentials.

## Explicitly Out Of Scope

- Hosted authentication.
- Cloud sync.
- Network time APIs.
- Live holiday APIs.
- Multi-user hosted service mode.
- Cross-device timer synchronization.

## Resolved Decisions

No high-impact product decisions are intentionally left open for scaffold work.

| Decision | Resolution |
| --- | --- |
| Default branch | `master` |
| First release tag | `v0.0.0` |
| Output default | JSON |
| Timezone default | UTC |
| Timezone identifiers | IANA names only |
| Timer store | Local SQLite |
| Holiday source | Offline bounded data |
| Holiday coverage | `2000..=2030` |
| MCP transports | stdio and streamable HTTP |
| HTTP default bind | Loopback |
| Release gate | Full production readiness review before tag |
