---
name: time-keep-timers
description: Use time-keep local SQLite timers for agent reminders, deadlines, overdue checks, tag filtering, and isolated timer-state workflows. Use when a task asks to set, inspect, list, delete, or smoke-test local time-keep timers.
license: MIT
allowed-tools: "Bash Read"
metadata:
  owner: "time-keep"
  version: "0.0.0"
---

# time-keep Timers Skill

## Mission

Manage local timers without touching unrelated user state unless the user
intends it.

## Required Workflow

1. Choose the data directory:
   - Use the user's default only when managing their real timers.
   - Use `TIME_KEEP_DATA_DIR="$(mktemp -d)"` for tests, examples, and smoke
     checks.
2. Set timers with absolute ISO/RFC3339 deadlines.
3. Use tags for filtering repeated workflows.
4. Verify with `timer get`, `timer list`, or `timer check`.
5. Report the data directory if timer state was mutated.

## Commands

```bash
data_dir="$(mktemp -d)"
TIME_KEEP_DATA_DIR="$data_dir" time-keep timer set deploy-window 2026-07-01T17:00:00-04:00 --tag ops --tag release
TIME_KEEP_DATA_DIR="$data_dir" time-keep timer get deploy-window
TIME_KEEP_DATA_DIR="$data_dir" time-keep timer list --tag ops
TIME_KEEP_DATA_DIR="$data_dir" time-keep timer check
TIME_KEEP_DATA_DIR="$data_dir" time-keep timer delete deploy-window
```

## Guardrails

- Do not use vague deadlines such as "tomorrow" in commands.
- Do not mutate the default timer database during validation.
- Do not imply cloud sync or multi-user timer state.
- Treat timer names, descriptions, deadlines, and tags as private local data.

## Storage

Default path:

```text
~/.local/share/time-keep/timers.db
```

Timers use SQLite WAL and private file permissions where supported.
