---
name: time-keep
description: Local-first agent clock with MCP tools + CLI. Current time, IANA timezone ops, date arithmetic, calendar queries, business days, offline holiday lookups (2000-2030), and SQLite timers. Use when a task needs reliable agent time context, timezone conversion, working-day counts, holiday-aware planning, date formatting, or local reminder timers.
---

# time-keep

Local-first MCP tools and CLI for deterministic time, calendar, and timer work. Prefer MCP when available; fall back to CLI.

## Tool Map

All MCP tools are prefixed `time-keep_`:

| Need | MCP Tool |
|------|----------|
| Current time | `current_time` |
| List timezones | `list_timezones` |
| Timezone details + DST | `timezone_info` |
| Convert between timezones | `convert_timezone` |
| Calendar fields (weekday, ISO week, etc.) | `calendar_query` |
| Date add/subtract | `date_arithmetic` |
| Date difference | `date_diff` |
| Parse/format dates | `date_format` |
| Holiday check or list | `holidays` |
| Business day counts | `business_days` |
| Set a timer | `timer_set` |
| Read a timer | `timer_get` |
| List timers by tag | `timer_list` |
| Delete a timer | `timer_delete` |
| Check overdue timers | `timer_check` |

## Guardrails

- With no configured default, the timezone is **UTC**. `now` and
  `current_time` can use a configured default when the caller omits one; pass
  an explicit IANA timezone for reproducible workflows.
- Holiday data is offline, bounded to **2000–2030**. Mention this when used.
- Timers use local SQLite at `~/.local/share/time-keep/timers.db`.
- Do not mutate the default timer database during tests; use `TIME_KEEP_DATA_DIR="$(mktemp -d)"`.
- Use IANA timezone names. Do not silently accept bare city names.
- Do not hide DST ambiguity — report ambiguous local datetimes as invalid.
- MCP tool failures surface as `isError: true` — inspect the JSON error before retrying.

## Output Standard

Include: tool/command used, input date, resolved timezone, date range, whether holiday data was used, and any error details.

## Sub-topics

- **Calendar & dates**: `time-keep calendar_query 2026-06-21`, `convert_timezone`, `date_arithmetic`, `date_diff`, `date_format`, `holidays`, `business_days`. See [references/calendar.md](references/calendar.md).
- **Timers**: `timer_set`, `timer_get`, `timer_list`, `timer_check`, `timer_delete`. See [references/timers.md](references/timers.md).
- **CLI transport**: When MCP is unavailable, use the `time-keep` binary. See [references/cli.md](references/cli.md).
- **Output contracts**: JSON (default), table, CSV, and error envelopes. See [references/output-contracts.md](references/output-contracts.md).
