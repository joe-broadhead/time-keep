---
name: time-keep
description: Use time-keep to answer current-time, timezone, calendar/date, holiday, business-day, timer, and MCP-tooling questions with deterministic local-first CLI or MCP results. Use when a task needs reliable agent time context, IANA timezone conversion, date arithmetic, local timers, offline holidays, or JSON/CSV/table output from the time-keep binary.
license: MIT
allowed-tools: "Bash Read"
metadata:
  owner: "time-keep"
  version: "0.0.0"
---

# time-keep Skill

## Mission

Use time-keep to turn ambiguous time and calendar questions into reproducible
CLI or MCP results with explicit timezones, absolute dates, and local-only
state.

## Required Workflow

1. Choose transport:
   - Prefer MCP when time-keep MCP tools are available in the client.
   - Otherwise use the `time-keep` CLI through Bash.
2. Make relative dates explicit before acting. If the user says "today",
   "tomorrow", or "next Friday", include the resolved absolute date in the
   answer.
3. Use IANA timezone names. Do not silently accept bare city names.
4. Prefer JSON output for evidence and automation.
5. Use `--data-dir` or `TIME_KEEP_DATA_DIR` for disposable timer work.
6. Report the command or tool used, the timezone assumptions, and any coverage
   limits that matter.

## Command Defaults

- Output: JSON
- Default timezone: UTC
- Holiday coverage: offline `2000..=2030`
- Timer state: local SQLite
- MCP HTTP: loopback only unless an authenticating proxy controls access

## Transport Selection

- MCP: read `references/transport-mcp.md`.
- CLI: read `references/transport-cli.md`.
- Output contracts: read `references/output-contracts.md`.

## Guardrails

- Do not imply hosted auth, cloud sync, network time APIs, or live holiday
  APIs.
- Do not use local system timezone as a hidden fallback.
- Do not mutate the user's default timer database during tests or exploratory
  work; use an isolated data directory.
- Do not use holiday-aware business days outside `2000..=2030`.
- Treat MCP tool failures as `isError: true` and inspect the JSON error text
  before retrying.

## Output Standard

When summarizing results for the user, include:

- tool or command used
- input date/datetime
- resolved timezone
- absolute date range when relevant
- whether holiday data was used
- any structured error details that explain a failure

## References

Load only what is needed:

- `references/transport-cli.md` for CLI command usage.
- `references/transport-mcp.md` for MCP tool usage.
- `references/output-contracts.md` for JSON, table, CSV, and error behavior.
