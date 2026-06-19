---
name: time-keep-calendar
description: Use time-keep for date arithmetic, timezone conversion, calendar queries, business-day counts, and offline holiday checks. Use when a task asks for exact dates, timezone normalization, working days, holiday-aware planning, or date formatting.
license: MIT
allowed-tools: "Bash Read"
metadata:
  owner: "time-keep"
  version: "0.0.0"
---

# time-keep Calendar Skill

## Mission

Answer calendar and timezone questions with absolute dates, explicit IANA
timezones, and documented holiday coverage.

## Required Workflow

1. Resolve relative dates to absolute dates before running commands.
2. Use IANA timezone names for timezone-aware work.
3. Use `calendar`, `calc`, `format`, and `convert` for date operations.
4. Use `holiday` and `biz` for holiday or business-day questions.
5. Mention when holiday data is bounded to `2000..=2030`.

## Commands

```bash
time-keep calendar 2026-06-18
time-keep calc add 2026-01-31 1 month
time-keep calc diff 2026-06-01 2026-06-18
time-keep format 2026-06-18T12:00:00Z --output-format rfc2822
time-keep convert 2026-06-18T12:00:00Z --from UTC --to Europe/Madrid
time-keep biz between 2026-12-24 2026-12-28 --country US --skip-holidays
```

## Guardrails

- Do not use bare city names as timezones.
- Do not assume holiday data exists outside `2000..=2030`.
- Do not hide DST ambiguity. Ambiguous or nonexistent local datetimes should be
  reported as invalid rather than guessed.
- Use JSON for evidence unless the user asks for table or CSV.

## Reporting

Include the input date, operation, result, timezone, and whether holidays were
included.
