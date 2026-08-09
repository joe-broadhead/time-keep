# Timers

Timers are the only persistent runtime state.

## Commands

```bash
time-keep timer set q3-planning 2026-07-01T17:00:00-04:00 --description "Q3 planning due" --tag work --tag planning
time-keep timer get q3-planning
time-keep timer list --tag work
time-keep timer check
time-keep timer delete q3-planning
```

## Storage

Default path:

```text
XDG_DATA_HOME/time-keep/timers.db
```

Fallback path:

```text
~/.local/share/time-keep/timers.db
```

Use `--data-dir` or `TIME_KEEP_DATA_DIR` to isolate agent runs:

```bash
TIME_KEEP_DATA_DIR="$(mktemp -d)" time-keep timer list
```

## Persistence Contract

- SQLite uses WAL.
- `busy_timeout=5000` protects concurrent first-run and write paths.
- `foreign_keys=ON` keeps tag joins consistent.
- Migrations use `PRAGMA user_version`.
- Unix data directories are created as `0700` and database files as `0600`
  where the platform allows it.
- Deadlines are stored in UTC.
- The original ISO/RFC3339 input and resolved offset are preserved for display.
- Timezone-less ISO datetimes default to UTC.
- Tags are trimmed, lowercased, deduplicated, sorted, and queryable.

## MCP Mapping

| CLI | MCP tool |
| --- | --- |
| `timer set` | `timer_set` |
| `timer get` | `timer_get` |
| `timer list` | `timer_list` |
| `timer delete` | `timer_delete` |
| `timer check` | `timer_check` |

Use isolated `--data-dir` values in MCP client configuration when an agent
should not read or mutate the user's default timer database.
